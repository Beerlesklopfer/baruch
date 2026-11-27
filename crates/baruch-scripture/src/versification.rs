// SPDX-License-Identifier: AGPL-3.0-or-later
//! Versifications as pure numbers: "German" (the versification of the reference check,
//! `data/versification.tsv`) and "KJV" (`data/versification-kjv.tsv`) from the embedded tables, since Round 17a
//! also all other versifications that the crate `sword-modules` 0.1.0 knows (Calvin, Catholic, Catholic2,
//! DarbyFr, KJVA, LXX, Leningrad, Luther, MT, NRSV, NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg), at run
//! time from its data (CC0). The tables German/KJV stay authoritative; the test `tests/versification.rs` checks
//! that sword-modules has the same numbers.
//!
//! Every versification contains only the 66 books of [`crate::books`] in their order (39 in the Old, 27 in the
//! New Testament); deuterocanonical books stay outside, and if a versification lacks a book (MT and Leningrad
//! have no New Testament), [`Versification::chapters_of`] gives `None`. Mapping between the versifications:
//! [`mapping`].

use std::sync::LazyLock;

use crate::books::{META, parse_table};

pub mod mapping;

/// Name of the German versification (as in `Versification=German` of a SWORD conf).
pub const GERMAN: &str = "German";
/// Name of the English versification (SWORD default if a conf names no `Versification`).
pub const KJV: &str = "KJV";

/// Number of books of the Old Testament in German and KJV (in every versification that has all 66 books).
pub const OLD_TESTAMENT_BOOKS: usize = 39;

/// A versification: per book (OSIS code, order as in [`crate::books`]) the verses per chapter.
#[derive(Debug, PartialEq, Eq)]
pub struct Versification {
    pub name: &'static str,
    books: Vec<(&'static str, Vec<u16>)>,
}

impl Versification {
    /// Books in canonical order with the verses per chapter (`[0]` = chapter 1).
    pub fn books(&self) -> impl Iterator<Item = (&'static str, &[u16])> {
        self.books.iter().map(|(id, v)| (*id, v.as_slice()))
    }

    /// Verses per chapter of a book (OSIS code).
    pub fn chapters_of(&self, book: &str) -> Option<&[u16]> {
        self.books.iter().find(|(id, _)| *id == book).map(|(_, v)| v.as_slice())
    }

    /// Number of verses of a chapter (from 1); `None` if book or chapter does not exist.
    pub fn verses_in(&self, book: &str, chapter: u16) -> Option<u16> {
        let chapters = self.chapters_of(book)?;
        chapter.checked_sub(1).and_then(|i| chapters.get(usize::from(i))).copied()
    }

    /// Number of all verses.
    pub fn total_verses(&self) -> u32 {
        self.books.iter().flat_map(|(_, v)| v.iter()).map(|&n| u32::from(n)).sum()
    }

    /// Number of verses from `start` to `end` (chapter, verse each; verse 0 = whole chapter, i.e. from verse 1 or
    /// up to the last verse); `None` for an unknown book or chapter, a verse beyond the chapter or an
    /// end before the start.
    pub fn count_range(&self, book: &str, start: (u16, u16), end: (u16, u16)) -> Option<u32> {
        let first_len = self.verses_in(book, start.0)?;
        let last_len = self.verses_in(book, end.0)?;
        let first = start.1.max(1);
        let last = if end.1 == 0 { last_len } else { end.1 };
        if first > first_len || last > last_len || (end.0, last) < (start.0, first) {
            return None;
        }
        if start.0 == end.0 {
            return Some(u32::from(last - first) + 1);
        }
        let middle: u32 = (start.0 + 1..end.0).map(|c| self.verses_in(book, c).map(u32::from)).sum::<Option<u32>>()?;
        Some(u32::from(first_len - first) + 1 + middle + u32::from(last))
    }
}

fn build(name: &'static str, table: &str) -> Versification {
    let table = parse_table(table).unwrap_or_else(|e| panic!("Verszählung {name} fehlerhaft: {e}"));
    let books = META
        .iter()
        .map(|(id, ..)| {
            let verses = table.iter().find(|(b, _)| b == id).map(|(_, v)| v.clone());
            (*id, verses.unwrap_or_else(|| panic!("Buch {id} fehlt in der Verszählung {name}")))
        })
        .collect();
    Versification { name, books }
}

static GERMAN_V11N: LazyLock<Versification> =
    LazyLock::new(|| build(GERMAN, include_str!("../data/versification.tsv")));
static KJV_V11N: LazyLock<Versification> = LazyLock::new(|| build(KJV, include_str!("../data/versification-kjv.tsv")));

/// A versification from the data of sword-modules: the 66 books in the order of [`META`], as far as present.
fn from_sword(v: &'static sword_modules::Versification) -> Versification {
    let books = META
        .iter()
        .filter_map(|(id, ..)| {
            let def = v.book_by_osis(id)?;
            let verses: Vec<u16> = (1..=def.chapters).map(|c| v.verse_count(def.id, c).map_or(0, u16::from)).collect();
            Some((*id, verses))
        })
        .collect();
    Versification { name: v.name(), books }
}

/// The versifications from sword-modules except German and KJV (those come from the tables).
static OTHERS: LazyLock<Vec<Versification>> = LazyLock::new(|| {
    sword_modules::Versification::all()
        .iter()
        .filter(|v| !v.name().eq_ignore_ascii_case(GERMAN) && !v.name().eq_ignore_ascii_case(KJV))
        .map(from_sword)
        .collect()
});

/// All versifications: German, then the others alphabetically (case does not matter), KJV from the table.
static ALL: LazyLock<Vec<&'static Versification>> = LazyLock::new(|| {
    let mut others: Vec<&'static Versification> = OTHERS.iter().collect();
    others.push(&KJV_V11N);
    others.sort_by_key(|v| v.name.to_lowercase());
    std::iter::once(&*GERMAN_V11N).chain(others).collect()
});

/// The versification for a name (spelling of sword-modules, e.g. "German", "KJV", "Luther", "NRSVA";
/// case and whitespace do not matter). Unknown names: `None`.
pub fn versification(name: &str) -> Option<&'static Versification> {
    let name = name.trim();
    ALL.iter().copied().find(|v| v.name.eq_ignore_ascii_case(name))
}

/// All versifications that Baruch knows: "German" first, then alphabetically.
pub fn all() -> impl Iterator<Item = &'static Versification> {
    ALL.iter().copied()
}

/// The names from [`all`] in the same order.
pub fn names() -> Vec<&'static str> {
    all().map(|v| v.name).collect()
}

/// The German versification of the reference check.
pub fn german() -> &'static Versification {
    &GERMAN_V11N
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_is_the_table_of_the_reference_check() {
        let g = german();
        for (book, (id, verses)) in crate::books().iter().zip(g.books()) {
            assert_eq!((book.id, book.verses), (id, verses), "gleiche Tabelle wie die Stellenprüfung");
        }
    }

    #[test]
    fn names_are_compared_without_case_and_unknown_ones_are_refused() {
        assert_eq!(versification("german").map(|v| v.name), Some(GERMAN), "klein geschrieben");
        assert_eq!(versification(" KJV ").map(|v| v.name), Some(KJV), "mit Leerraum");
        assert_eq!(versification("luther").map(|v| v.name), Some("Luther"), "Schreibweise von sword-modules");
        assert!(versification("Phantasie").is_none(), "erfundene Zählung bleibt unbekannt");
    }

    #[test]
    fn count_range_counts_across_chapters_and_whole_chapters() {
        let g = german();
        assert_eq!(g.count_range("John", (3, 16), (3, 16)), Some(1), "ein Vers");
        assert_eq!(g.count_range("John", (3, 16), (4, 2)), Some(21 + 2), "Joh 3 hat 36 Verse: 16–36 und 4,1–2");
        assert_eq!(g.count_range("Ps", (23, 0), (23, 0)), Some(6), "ganzes Kapitel Ps 23");
        assert_eq!(g.count_range("Ps", (3, 0), (4, 0)), Some(9 + 9), "zwei ganze Kapitel (deutsch mit Überschrift)");
        assert_eq!(g.count_range("John", (3, 40), (3, 41)), None, "Vers jenseits des Kapitels");
        assert_eq!(g.count_range("John", (4, 1), (3, 1)), None, "Ende vor dem Anfang");
        assert_eq!(g.count_range("Xyz", (1, 1), (1, 1)), None, "unbekanntes Buch");
    }

    #[test]
    fn all_and_names_put_german_first() {
        assert_eq!(names().first(), Some(&GERMAN), "German zuerst: {:?}", names());
        assert!(names().contains(&KJV), "KJV dabei: {:?}", names());
        assert!(std::ptr::eq(versification(KJV).unwrap(), &*KJV_V11N), "KJV aus der eingebetteten Tabelle");
        let rest: Vec<String> = names()[1..].iter().map(|n| n.to_lowercase()).collect();
        assert!(rest.windows(2).all(|w| w[0] <= w[1]), "übrige alphabetisch: {rest:?}");
        let mt = versification("MT").unwrap();
        assert_eq!((mt.chapters_of("Ps").map(<[u16]>::len), mt.chapters_of("John")), (Some(150), None), "MT ohne NT");
        assert_eq!(all().count(), names().len(), "dieselbe Liste");
    }
}
