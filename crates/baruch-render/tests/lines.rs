// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_typst::line_at` counts lines exactly like Typst itself (`typst::syntax::Lines`): otherwise
//! messages of the preview would point to the wrong section.

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
