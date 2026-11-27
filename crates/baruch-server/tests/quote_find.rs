// SPDX-License-Identifier: AGPL-3.0-or-later
//! Follow-up fix Round 17: mapping quotation → typeset view in the widget „scripture-check“ (`web/src/quote_find.ts`).
//! The compiled pure functions run under Node (the build needs Node for `tsc` anyway); what is checked is the delivered
//! version from `baruch_server::SCRIPTS`.

use std::process::Command;

use serde_json::{Value, json};

/// Calls `placeQuotes(haystack, marks)` or `findAll`/`plainQuote` in Node and returns the result as JSON.
fn run(expr: &str, input: Value) -> Value {
    let (_, module) = baruch_server::SCRIPTS
        .iter()
        .find(|(name, _)| *name == "quote_find.js")
        .expect("quote_find.js wird ausgeliefert");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("quote_find.mjs"), module).unwrap();
    let script = format!(
        "import * as q from './quote_find.mjs';\nconst input = {input};\nconsole.log(JSON.stringify({expr}));\n"
    );
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = Command::new("node").arg("probe.mjs").current_dir(dir.path()).output().expect("node läuft (wie für tsc)");
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Text at the places found.
fn texts(haystack: &str, places: &Value) -> Vec<Option<String>> {
    let chars: Vec<u16> = haystack.encode_utf16().collect();
    places
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            p.as_array().map(|p| {
                let (s, e) = (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize);
                String::from_utf16(&chars[s..e]).unwrap()
            })
        })
        .collect()
}

#[test]
fn only_whole_words_are_found() {
    let found = run(
        "q.findAll(input.h, input.n)",
        json!({ "h": "Der Hirte sang. Die Hirtenflöte schwieg. hirte", "n": "Hirte" }),
    );
    assert_eq!(found, json!([[4, 9]]), "„Hirtenflöte“ enthält „Hirte“ nur als Teil eines Wortes; Groß/Klein zählt");
    let found = run("q.findAll(input.h, input.n)", json!({ "h": "Fürchtet euch nicht!", "n": "euch nicht" }));
    assert_eq!(found, json!([[9, 19]]), "Satzzeichen danach ist eine Wortgrenze");
}

#[test]
fn typst_markup_is_removed_before_searching() {
    let plain =
        run("q.plainQuote(input)", json!("Das _Licht_ scheint in der *Finsternis*, \\#1 und #emph[nicht] mehr"));
    assert_eq!(
        plain,
        json!("Das Licht scheint in der Finsternis, #1 und nicht mehr"),
        "Auszeichnung des Quelltexts steht im Satzbild nicht"
    );
    let h = "Er sagte: „Das Licht scheint“ und ging.";
    let places = run(
        "q.placeQuotes(input.h, input.m)",
        json!({ "h": h, "m": [{ "quote": "Das _Licht_ scheint", "quote_span": [14, 35] }] }),
    );
    assert_eq!(texts(h, &places), vec![Some("Das Licht scheint".to_owned())], "Zitat mit Auszeichnung gefunden");
}

#[test]
fn the_same_wording_twice_goes_to_its_own_place_in_order_of_the_source() {
    // Twice the same wording, once rightly, once wrongly referenced: every passage gets exactly its place found (order
    // of quote_span), not both colours at both places.
    let h = "Zuerst „Fürchtet euch nicht“ (Lk 2,10), später „Fürchtet euch nicht“ (Joh 99,1). Fürchtet euch nicht.";
    let marks = json!([
        { "quote": "Fürchtet euch nicht", "quote_span": [90, 110] },
        { "quote": "Fürchtet euch nicht", "quote_span": [10, 30] },
    ]);
    let places = run("q.placeQuotes(input.h, input.m)", json!({ "h": h, "m": marks }));
    let chars: Vec<char> = h.chars().collect();
    let starts: Vec<u64> = places.as_array().unwrap().iter().map(|p| p[0].as_u64().unwrap()).collect();
    assert!(starts[1] < starts[0], "das Zitat mit dem früheren quote_span bekommt die frühere Fundstelle: {places}");
    let quoted_at = |s: u64| chars[s as usize - 1] == '„';
    assert!(
        quoted_at(starts[0]) && quoted_at(starts[1]),
        "zugeordnet werden die Fundstellen in Anführungszeichen, nicht der ungezeichnete Satz am Ende: {places}"
    );
}

#[test]
fn an_ambiguous_quote_stays_unmarked_and_one_quote_for_two_references_shares_its_place() {
    let h = "Licht und Licht und Licht";
    let places =
        run("q.placeQuotes(input.h, input.m)", json!({ "h": h, "m": [{ "quote": "Licht", "quote_span": [3, 8] }] }));
    assert_eq!(places, json!([null]), "drei Fundstellen, ein Zitat, keine in Anführungszeichen: nicht eindeutig");
    let h = "Er sprach „Licht werde“ zu allen.";
    let places = run(
        "q.placeQuotes(input.h, input.m)",
        json!({ "h": h, "m": [{ "quote": "Licht werde", "quote_span": [5, 17] }, { "quote": "Licht werde", "quote_span": [5, 17] }, { "quote": null }] }),
    );
    assert_eq!(
        texts(h, &places),
        vec![Some("Licht werde".to_owned()), Some("Licht werde".to_owned()), None],
        "derselbe Bereich = dasselbe Zitat; ohne Zitat keine Fundstelle"
    );
}
