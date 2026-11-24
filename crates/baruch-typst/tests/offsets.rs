// SPDX-License-Identifier: AGPL-3.0-or-later
//! `join_with_offsets` und `line_at` (Runde 8, Typst-Vorschau): Bytebereiche der Abschnitte im
//! Export und Zeilen wie in Typst.

use baruch_typst::{join, join_with_offsets, line_at};

#[test]
fn join_with_offsets_is_byte_identical_to_join_and_ranges_cover_each_body() {
    let template = "#set text(lang: \"de\")\n";
    let bodies = ["= Eins\nÄpfel\n", "", "== Zwei\nBirnen"];
    let (joined, ranges) = join_with_offsets(template, bodies);
    assert_eq!(joined, join(template, bodies), "die Verkettung muss byte-gleich mit join sein");
    assert_eq!(ranges.len(), 3, "je Text ein Bereich, auch für den leeren");
    for (range, body) in ranges.iter().zip(bodies) {
        assert_eq!(&joined[range.clone()], body, "der Bereich muss genau den Text treffen");
    }
    assert_eq!(ranges[0].start, template.len(), "der erste Text beginnt hinter der Vorlage");
    assert_eq!(ranges[1], ranges[0].end..ranges[0].end, "ein leerer Text hat einen leeren Bereich");
}

#[test]
fn line_at_counts_lines_like_typst() {
    let text = "a\nb\r\nc\u{2028}d\re";
    assert_eq!(line_at(text, 0), 1, "Anfang ist Zeile 1");
    assert_eq!(line_at(text, 2), 2, "hinter \\n beginnt Zeile 2");
    assert_eq!(line_at(text, 4), 2, "zwischen \\r und \\n bleibt es Zeile 2");
    assert_eq!(line_at(text, 5), 3, "\\r\\n zählt als ein Zeilenende");
    assert_eq!(line_at(text, text.find('d').unwrap()), 4, "U+2028 beendet eine Zeile");
    assert_eq!(line_at(text, text.len()), 5, "einzelnes \\r beendet eine Zeile");
    assert_eq!(line_at(text, text.len() + 10), 5, "hinter dem Ende zählt wie das Ende");
}
