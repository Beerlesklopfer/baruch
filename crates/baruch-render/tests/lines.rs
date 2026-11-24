// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_typst::line_at` zählt Zeilen genau wie Typst selbst (`typst::syntax::Lines`): sonst
//! zeigten Meldungen der Vorschau auf den falschen Abschnitt.

use typst::syntax::Lines;

#[test]
fn line_at_agrees_with_the_line_numbers_of_typst() {
    let text = "#set text(lang: \"de\")\r\n= Eins\nÄpfel\u{85}und\u{2028}Birnen\x0Bx\x0Cy\r\r\n== Zwei\nEnde";
    let lines = Lines::new(text.to_owned());
    for byte in (0..=text.len()).filter(|&b| text.is_char_boundary(b)) {
        let typst = lines.byte_to_line(byte).expect("Stelle im Text") + 1;
        assert_eq!(baruch_typst::line_at(text, byte), typst, "Byte {byte}: Zeile muss mit Typst übereinstimmen");
    }
}
