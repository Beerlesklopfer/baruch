// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23a, contract §4 (quote tool): measurement with the embedded Typst whether an **empty** attribution
//! `#quote(block: true, attribution: [])[…]` sets a dash line. On that depends what the button „Zitat“ inserts:
//! if Typst sets the line, the button must not preset an empty `attribution` (it would be in the book if the
//! author gives no details), but leaves room in a following line of its own. All texts are freely invented.

use baruch_render::{PlacedText, Source, page_texts};

fn texts(body: &str) -> Vec<PlacedText> {
    let main = format!("#set page(width: 120mm, height: 80mm, margin: 10mm)\n#set text(size: 10pt)\n{body}\n");
    let pages = page_texts(&Source { main, files: Vec::new() }).expect("der erfundene Text muss sich setzen lassen");
    pages.into_iter().flatten().collect()
}

/// Lowest baseline (y in pt) of all typeset text pieces.
fn lowest(texts: &[PlacedText]) -> f64 {
    texts.iter().map(|t| t.y_pt).fold(0.0, f64::max)
}

#[test]
fn an_empty_attribution_still_sets_a_dash_line_so_the_ribbon_must_not_insert_one() {
    let plain = texts("#quote(block: true)[Der Kran hebt nur, was man ihm anhängt.]");
    let empty = texts("#quote(block: true, attribution: [])[Der Kran hebt nur, was man ihm anhängt.]");
    let filled = texts("#quote(block: true, attribution: [Hafenordnung 9])[Der Kran hebt nur, was man ihm anhängt.]");

    let dash = |list: &[PlacedText]| list.iter().any(|t| t.text.contains('—'));
    assert!(!dash(&plain), "ohne `attribution` setzt Typst keinen Geviertstrich: {plain:?}");
    assert!(dash(&filled), "mit Angabe steht sie hinter einem Geviertstrich (Gegenprobe der Messung): {filled:?}");
    assert!(
        dash(&empty),
        "MESSUNG: eine leere `attribution: []` setzt trotzdem die Strichzeile – darum fügt der Knopf „Zitat“ \
         `#quote(block: true)[]` mit eigener Folgezeile ein (ribbon::QUOTE_SNIPPET), nicht `attribution: []`: {empty:?}"
    );
    assert!(
        lowest(&empty) > lowest(&plain) + 5.0,
        "die Strichzeile ist eine eigene Zeile unter dem Zitat (tiefste Grundlinie {} pt gegen {} pt ohne Angabe)",
        lowest(&empty),
        lowest(&plain)
    );
}

#[test]
fn a_line_directly_after_a_block_quote_is_set_as_its_own_paragraph_below_it() {
    // `baruch_wysiwyg::parse` reads it the same way since Round 23a: the line after `]` is a paragraph of its own.
    let glued = texts("#quote(block: true)[Der Kran hebt nur, was man ihm anhängt.]\n– Hafenordnung 9");
    let quote = glued.iter().find(|t| t.text.contains("Kran")).expect("Zitat gesetzt");
    let reference = glued.iter().find(|t| t.text.contains("Hafenordnung")).expect("Angabe gesetzt");
    assert!(
        reference.y_pt > quote.y_pt + 5.0,
        "die Angabe steht in einer eigenen Zeile unter dem Zitat, nicht in dessen Zeile ({} pt gegen {} pt)",
        reference.y_pt,
        quote.y_pt
    );
    assert!(
        reference.x_pt < quote.x_pt,
        "und sie beginnt am Satzspiegel, nicht im Einzug des Zitatblocks: also ein eigener Absatz ({} pt gegen {} pt)",
        reference.x_pt,
        quote.x_pt
    );
}
