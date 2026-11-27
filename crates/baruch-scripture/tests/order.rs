// SPDX-License-Identifier: AGPL-3.0-or-later
//! Biblical book order and sort keys (Round 31b, index of Bible references).

use baruch_scripture::{RefStatus, book_order, books, find_book, parse, scan};

#[test]
fn books_are_numbered_in_canonical_order() {
    assert_eq!(book_order(find_book("1. Mose").expect("Genesis")), 1, "1. Mose ist das erste Buch");
    assert_eq!(book_order(find_book("Mal").expect("Maleachi")), 39, "Maleachi schließt das Alte Testament");
    assert_eq!(book_order(find_book("Mt").expect("Matthäus")), 40, "Matthäus eröffnet das Neue Testament");
    assert_eq!(book_order(find_book("Offb").expect("Offenbarung")), 66, "Offenbarung ist das letzte Buch");
    let orders: Vec<u8> = books().iter().map(book_order).collect();
    assert_eq!(orders, (1..=66).collect::<Vec<u8>>(), "lückenlos 1 bis 66 in der Reihenfolge von books()");
}

#[test]
fn sort_keys_follow_the_biblical_order_not_the_alphabet() {
    let key = |r: &str| {
        let found = parse(r).into_iter().next().expect("lesbar");
        assert_eq!(found.status, RefStatus::Valid, "{r} gibt es");
        found.passage.expect("Stelle").sort_key()
    };
    let mut refs = vec!["Röm 8,28", "Joh 3,16", "1. Mose 1,1", "Ps 23", "Ps 23,1", "Joh 3,5", "Joh 10,11", "Amos 5,24"];
    refs.sort_by_key(|r| key(r));
    assert_eq!(
        refs,
        vec!["1. Mose 1,1", "Ps 23", "Ps 23,1", "Amos 5,24", "Joh 3,5", "Joh 3,16", "Joh 10,11", "Röm 8,28"],
        "Bücher in Bibelordnung, Kapitel und Verse numerisch (10 nach 3), ganzes Kapitel vor seinen Versen"
    );
    assert_eq!(key("Joh 3,16"), "43.003.016-003.016", "feste Breite: Buch 43, Kapitel 3, Vers 16");
}

#[test]
fn chapter_verse_drops_the_book() {
    let cv = |text: &str| scan(text)[0].passage.expect("Stelle").chapter_verse();
    assert_eq!(cv("wie in Joh 3,16 steht"), "3,16", "Vers ohne Buch");
    assert_eq!(cv("lies 1. Kön 3,16-28 heute"), "3,16–28", "Bereich mit Gedankenstrich wie normalized");
    assert_eq!(cv("Psalm 23 tröstet"), "23", "ganzes Kapitel");
}
