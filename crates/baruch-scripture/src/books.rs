// SPDX-License-Identifier: AGPL-3.0-or-later
//! The 66 books: OSIS code, name according to Schlachter, common abbreviation and further spellings.
//! The verse counts come from `data/versification.tsv` (see [`crate::SOURCE`]).

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::Book;

/// (OSIS, name, abbreviation, further spellings). Numbered books are compared without full stop and space
/// ([`key`]): „1. Kön“, „1 Kön“, „1.Kön“ and „1Kön“ are the same key.
pub(crate) const META: [(&str, &str, &str, &[&str]); 66] = [
    ("Gen", "1. Mose", "1. Mo", &["1. Mos", "Genesis", "Gen", "Gn"]),
    ("Exod", "2. Mose", "2. Mo", &["2. Mos", "Exodus", "Ex", "Exod"]),
    ("Lev", "3. Mose", "3. Mo", &["3. Mos", "Levitikus", "Leviticus", "Lev"]),
    ("Num", "4. Mose", "4. Mo", &["4. Mos", "Numeri", "Num"]),
    ("Deut", "5. Mose", "5. Mo", &["5. Mos", "Deuteronomium", "Dtn", "Deut"]),
    ("Josh", "Josua", "Jos", &[]),
    ("Judg", "Richter", "Ri", &["Richt"]),
    ("Ruth", "Ruth", "Rt", &["Rut"]),
    ("1Sam", "1. Samuel", "1. Sam", &["1. Sa"]),
    ("2Sam", "2. Samuel", "2. Sam", &["2. Sa"]),
    ("1Kgs", "1. Könige", "1. Kön", &["1. Kö", "1. Kg", "1. Kge"]),
    ("2Kgs", "2. Könige", "2. Kön", &["2. Kö", "2. Kg", "2. Kge"]),
    ("1Chr", "1. Chronik", "1. Chr", &["1. Chron"]),
    ("2Chr", "2. Chronik", "2. Chr", &["2. Chron"]),
    ("Ezra", "Esra", "Esr", &[]),
    ("Neh", "Nehemia", "Neh", &[]),
    ("Esth", "Esther", "Est", &["Esth", "Ester"]),
    ("Job", "Hiob", "Hi", &["Hiob", "Ijob", "Job"]),
    ("Ps", "Psalmen", "Ps", &["Psalm", "Psa", "Pss"]),
    ("Prov", "Sprüche", "Spr", &["Sprichwörter", "Sprueche"]),
    ("Eccl", "Prediger", "Pred", &["Prd", "Kohelet", "Koh"]),
    ("Song", "Hohelied", "Hld", &["Hoheslied", "Hl"]),
    ("Isa", "Jesaja", "Jes", &[]),
    ("Jer", "Jeremia", "Jer", &[]),
    ("Lam", "Klagelieder", "Klgl", &["Klg", "Klagel"]),
    ("Ezek", "Hesekiel", "Hes", &["Ezechiel", "Ez"]),
    ("Dan", "Daniel", "Dan", &["Da", "Dn"]),
    ("Hos", "Hosea", "Hos", &[]),
    ("Joel", "Joel", "Joel", &["Jl"]),
    ("Amos", "Amos", "Am", &[]),
    ("Obad", "Obadja", "Ob", &["Obd"]),
    ("Jonah", "Jona", "Jona", &["Jon"]),
    ("Mic", "Micha", "Mi", &["Mich"]),
    ("Nah", "Nahum", "Nah", &[]),
    ("Hab", "Habakuk", "Hab", &[]),
    ("Zeph", "Zephanja", "Zeph", &["Zef", "Zefanja"]),
    ("Hag", "Haggai", "Hag", &[]),
    ("Zech", "Sacharja", "Sach", &[]),
    ("Mal", "Maleachi", "Mal", &[]),
    ("Matt", "Matthäus", "Mt", &["Matth", "Mat", "Matthaeus"]),
    ("Mark", "Markus", "Mk", &["Mark", "Mr"]),
    ("Luke", "Lukas", "Lk", &["Luk"]),
    ("John", "Johannes", "Joh", &[]),
    ("Acts", "Apostelgeschichte", "Apg", &["Apostelg"]),
    ("Rom", "Römer", "Röm", &["Roemer", "Röm", "Rm"]),
    ("1Cor", "1. Korinther", "1. Kor", &[]),
    ("2Cor", "2. Korinther", "2. Kor", &[]),
    ("Gal", "Galater", "Gal", &[]),
    ("Eph", "Epheser", "Eph", &[]),
    ("Phil", "Philipper", "Phil", &["Php"]),
    ("Col", "Kolosser", "Kol", &[]),
    ("1Thess", "1. Thessalonicher", "1. Thess", &["1. Th", "1. Thes"]),
    ("2Thess", "2. Thessalonicher", "2. Thess", &["2. Th", "2. Thes"]),
    ("1Tim", "1. Timotheus", "1. Tim", &[]),
    ("2Tim", "2. Timotheus", "2. Tim", &[]),
    ("Titus", "Titus", "Tit", &[]),
    ("Phlm", "Philemon", "Phlm", &["Philem", "Phm"]),
    ("Heb", "Hebräer", "Hebr", &["Heb", "Hebraeer"]),
    ("Jas", "Jakobus", "Jak", &[]),
    ("1Pet", "1. Petrus", "1. Petr", &["1. Pt", "1. Pet"]),
    ("2Pet", "2. Petrus", "2. Petr", &["2. Pt", "2. Pet"]),
    ("1John", "1. Johannes", "1. Joh", &["1. Jo"]),
    ("2John", "2. Johannes", "2. Joh", &["2. Jo"]),
    ("3John", "3. Johannes", "3. Joh", &["3. Jo"]),
    ("Jude", "Judas", "Jud", &["Jd"]),
    ("Rev", "Offenbarung", "Offb", &["Off", "Apk", "Offenb"]),
];

/// Abbreviations that are also ordinary words in running text („am“, „ob“, „mi“, „Mal“ …): [`crate::scan`] takes
/// them only with a verse („Am 3,7“), not as a whole chapter („Am 3. Tag“).
pub(crate) const AMBIGUOUS: [&str; 9] = ["am", "mi", "ob", "hab", "ri", "tit", "da", "hi", "mal"];

/// Comparison key of a spelling: lower case, without full stops and whitespace, umlauts stay.
pub(crate) fn key(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace() && *c != '.').flat_map(char::to_lowercase).collect()
}

/// Verse counts per book from the embedded table (order as in [`META`]).
static COUNTS: LazyLock<Vec<Vec<u16>>> = LazyLock::new(|| {
    let table = parse_table(include_str!("../data/versification.tsv")).expect("data/versification.tsv fehlerhaft");
    META.iter()
        .map(|(id, ..)| table.iter().find(|(b, _)| b == id).map(|(_, v)| v.clone()).expect("Buch fehlt in der Tabelle"))
        .collect()
});

pub(crate) static BOOKS: LazyLock<Vec<Book>> = LazyLock::new(|| {
    META.iter()
        .zip(COUNTS.iter())
        .map(|((id, name, abbr, _), verses)| Book { id, name, abbr, verses: verses.as_slice() })
        .collect()
});

static INDEX: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (i, (id, name, abbr, more)) in META.iter().enumerate() {
        for spelling in [*id, *name, *abbr].into_iter().chain(more.iter().copied()) {
            let previous = map.insert(key(spelling), i);
            assert!(previous.is_none_or(|p| p == i), "Schreibweise {spelling} doppelt vergeben");
        }
    }
    map
});

/// Book for a spelling (case, full stops and whitespace do not matter).
pub(crate) fn lookup(text: &str) -> Option<&'static Book> {
    INDEX.get(&key(text)).map(|&i| &BOOKS[i])
}

/// `Buch\tKapitel\tVerse` per line, `#` starts comments. Chapters must ascend per book without gaps from 1,
/// every chapter has at least one verse.
pub(crate) fn parse_table(text: &str) -> Result<Vec<(String, Vec<u16>)>, String> {
    let mut books: Vec<(String, Vec<u16>)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let (Some(book), Some(chapter), Some(verses), None) = (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            return Err(format!("Zeile {}: drei Spalten erwartet", n + 1));
        };
        let chapter: usize = chapter.parse().map_err(|_| format!("Zeile {}: Kapitel ist keine Zahl", n + 1))?;
        let verses: u16 = verses.parse().map_err(|_| format!("Zeile {}: Verse ist keine Zahl", n + 1))?;
        if verses == 0 {
            return Err(format!("Zeile {}: Kapitel ohne Verse", n + 1));
        }
        if books.last().is_none_or(|(b, _)| b != book) {
            if books.iter().any(|(b, _)| b == book) {
                return Err(format!("Zeile {}: Buch {book} kommt zweimal vor", n + 1));
            }
            books.push((book.to_owned(), Vec::new()));
        }
        let list = &mut books.last_mut().expect("eben angelegt").1;
        if chapter != list.len() + 1 {
            return Err(format!("Zeile {}: Kapitel {chapter} von {book} nicht in Folge", n + 1));
        }
        list.push(verses);
    }
    Ok(books)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_compare_without_dots_spaces_and_case() {
        assert_eq!(key("1. Kön"), "1kön", "Punkt und Leerzeichen fallen weg");
        assert_eq!(key("1.Kön"), key("1 Kön"), "mit und ohne Leerzeichen gleich");
        assert_eq!(key("PSALM"), "psalm", "Großschreibung egal");
    }

    #[test]
    fn the_table_parser_refuses_gaps_and_garbage() {
        assert!(parse_table("# Kopf\nGen\t1\t31\nGen\t2\t25\n").is_ok(), "zwei Kapitel in Folge");
        assert!(parse_table("Gen\t2\t25\n").is_err(), "Kapitel 1 fehlt");
        assert!(parse_table("Gen\t1\t0\n").is_err(), "Kapitel ohne Verse");
        assert!(parse_table("Gen\t1\tx\n").is_err(), "keine Zahl");
        assert!(parse_table("Gen\t1\t31\nExod\t1\t22\nGen\t2\t25\n").is_err(), "Buch zweimal");
    }

    #[test]
    fn every_spelling_points_to_exactly_one_book() {
        // INDEX checks on construction that no spelling means two books.
        assert!(INDEX.len() > 66 * 2, "mehr als Name und Abkürzung je Buch: {}", INDEX.len());
        for word in AMBIGUOUS {
            assert!(INDEX.contains_key(word), "{word} ist eine Abkürzung eines Buchs");
        }
    }
}
