// SPDX-License-Identifier: AGPL-3.0-or-later
//! Querverweise (Runde 30a): Sprungmarken `<kN>` an Überschriften (mit und ohne eigenes Label), Verweise
//! `#link(<kN>)[…]`, Einlesen (Sprungmarken entfernen) und Umsetzen der Schlüssel. Alle Texte frei erfunden.

use std::borrow::Cow;
use std::collections::HashMap;

use baruch_typst::xref::{
    XrefUse, anchor_line, find_xrefs, label_heading, label_section, remap_keys, strip_key_label, strip_key_labels,
    strip_section, xref_snippet,
};
use baruch_typst::{part_line, unnumbered_line};

#[test]
fn a_plain_heading_gets_the_label_behind_its_title_and_before_a_comment() {
    assert_eq!(label_heading("= Leuchtturm\n", 5), "= Leuchtturm <k5>\n", "Label am Zeilenende vor dem Umbruch");
    assert_eq!(label_heading("== Wachturm", 12), "== Wachturm <k12>", "letzte Zeile ohne Umbruch");
    assert_eq!(
        label_heading("=== Brückenbuch   // Notiz\n", 7),
        "=== Brückenbuch <k7>   // Notiz\n",
        "ein Kommentar gehört nicht zum Titel: das Label steht davor (sonst wäre es auskommentiert)"
    );
    assert_eq!(
        label_heading("= Siehe https://example.org\n", 3),
        "= Siehe https://example.org <k3>\n",
        "`//` in einem Link ist kein Kommentar"
    );
    assert_eq!(label_heading("Fließtext\n", 3), "Fließtext\n", "keine Überschrift: unverändert");
    assert_eq!(label_heading("==== tief\n", 3), "==== tief\n", "Ebene 4 ist keine Gliederung");
}

#[test]
fn a_heading_with_its_own_label_gets_an_anchor_line_below() {
    let part = part_line("Vom Säen");
    assert_eq!(
        label_heading(&part, 9),
        format!("{part}{}\n", anchor_line(9)),
        "Teil: Typst kennt nur ein Label je Element, darum eine Ankerzeile darunter"
    );
    assert_eq!(anchor_line(9), "#metadata(\"baruch-key\") <k9>", "Form der Ankerzeile");
    let unnumbered = unnumbered_line(2, "Vorwort");
    assert_eq!(label_heading(&unnumbered, 4), format!("{unnumbered}{}\n", anchor_line(4)), "ohne Nummer ebenso");
    assert_eq!(
        label_heading("= Hafen <hafen>\n", 4),
        format!("= Hafen <hafen>\n{}\n", anchor_line(4)),
        "eigenes Label des Menschen bleibt an der Überschrift"
    );
    assert_eq!(
        label_heading("= Hafen <hafen>", 4),
        format!("= Hafen <hafen>\n{}", anchor_line(4)),
        "letzte Zeile ohne Umbruch: Umbruch davor, keiner dahinter"
    );
}

#[test]
fn a_label_line_below_the_heading_keeps_belonging_to_it() {
    let body = "= Hafen\n<hafen>\nDie Boote liegen still.\n";
    let labelled = label_section(body, 8);
    assert_eq!(
        labelled,
        format!("= Hafen\n<hafen>\n{}\nDie Boote liegen still.\n", anchor_line(8)),
        "das Label der Zeile darunter hängt an der Überschrift; die Ankerzeile steht dahinter, sonst bekäme sie es"
    );
    assert_eq!(strip_section(&labelled), (Cow::Borrowed(body), Some(8)), "Rückweg byte-gleich");
}

#[test]
fn labelling_and_stripping_a_section_round_trips_byte_for_byte() {
    let cases = [
        "= Leuchtturm\n\nDer Leuchtturm blinkt.\n".to_owned(),
        "== Wachturm // alt\nText.".to_owned(),
        "= Turm".to_owned(),
        format!("{}\n", part_line("Vom Ernten")),
        part_line("Ohne Umbruch"),
        format!("{}Ein Wort vorab.\n", unnumbered_line(1, "Vorwort")),
        "= Hafen <hafen>\nText.\n".to_owned(),
        "= Hafen\n<hafen>".to_owned(),
        "Kein Titel\n".to_owned(),
        String::new(),
    ];
    for body in &cases {
        let labelled = label_section(body, 42);
        let (back, key) = strip_section(&labelled);
        assert_eq!(back, body.as_str(), "{body:?}: Einlesen stellt den Datenbanktext wieder her (über {labelled:?})");
        let expect = if body.is_empty() || body.starts_with("Kein") { None } else { Some(42) };
        assert_eq!(key, expect, "{body:?}: Schlüssel aus der Sprungmarke");
        assert_eq!(strip_key_labels(&labelled), body.as_str(), "{body:?}: ganze Datei ebenso");
    }
}

#[test]
fn the_parser_reads_the_same_title_with_and_without_the_label() {
    let source = format!(
        "#set page(width: 10cm)\n{}{}Text.\n{}Mehr.\n",
        label_section(&format!("{}\n", part_line("Teil")), 1),
        label_section("= Leuchtturm\n", 2),
        label_section("== Wachturm // Notiz\n", 3)
    );
    let doc = baruch_typst::parse(&source).unwrap();
    let titles: Vec<&str> = doc.headings.iter().map(|h| h.title.as_str()).collect();
    assert_eq!(titles, ["Teil", "Leuchtturm", "Wachturm"], "das Label gehört nicht zum Titel: {source}");
}

#[test]
fn strip_key_label_removes_only_key_labels() {
    assert_eq!(strip_key_label("= Turm <k5>\n"), (Cow::Owned("= Turm\n".to_owned()), Some(5)), "Sprungmarke weg");
    assert_eq!(
        strip_key_label("= Turm <k5> // Notiz\n"),
        (Cow::Owned("= Turm // Notiz\n".to_owned()), Some(5)),
        "vor dem Kommentar"
    );
    assert_eq!(strip_key_label("#metadata(\"baruch-key\") <k7>\n"), (Cow::Borrowed(""), Some(7)), "Ankerzeile weg");
    for keep in ["= Turm <hafen>\n", "= Turm <k05>\n", "Text <k5>\n", "#metadata(\"anders\") <k7>\n"] {
        assert_eq!(strip_key_label(keep), (Cow::Borrowed(keep), None), "{keep:?}: keine Sprungmarke von Baruch");
    }
}

#[test]
fn find_xrefs_reads_both_forms_and_nested_brackets() {
    let plain = xref_snippet(103, "die Einleitung", false);
    let paged = xref_snippet(7, "das *zweite* Kapitel", true);
    let text = format!("Siehe {plain} und {paged}; nicht \\#link(<k9>)[maskiert], nicht #link(<anders>)[fremd].");
    let found = find_xrefs(&text);
    assert_eq!(
        found,
        vec![
            XrefUse {
                start: text.find("#link(<k103>)").unwrap(),
                end: text.find("#link(<k103>)").unwrap() + plain.len(),
                key: 103,
                text: "die Einleitung".into(),
                page: false,
            },
            XrefUse {
                start: text.find("#link(<k7>)").unwrap(),
                end: text.find("#link(<k7>)").unwrap() + paged.len(),
                key: 7,
                text: "das *zweite* Kapitel".into(),
                page: true,
            },
        ],
        "beide Formen mit Bytebereich; maskiertes `\\#link` und fremde Labels zählen nicht"
    );
    let nested = "#link(<k4>)[Kapitel #emph[Brücke] \\] Ende]";
    assert_eq!(find_xrefs(nested)[0].text, "Kapitel #emph[Brücke] \\] Ende", "Klammern geschachtelt, `\\]` maskiert");
    assert!(find_xrefs("#link(<k4>)[offen").is_empty(), "ohne schließende Klammer kein Verweis");
}

#[test]
fn remap_keys_moves_references_and_the_page_counter() {
    let text = format!(
        "A {} B {} C {}",
        xref_snippet(1, "eins", true),
        xref_snippet(2, "zwei", false),
        xref_snippet(3, "drei", false)
    );
    let map = HashMap::from([(1, 101), (2, 102)]);
    assert_eq!(
        remap_keys(&text, &map),
        format!(
            "A {} B {} C {}",
            xref_snippet(101, "eins", true),
            xref_snippet(102, "zwei", false),
            xref_snippet(3, "drei", false)
        ),
        "alte Schlüssel → neue, auch im Seitenzahl-Zusatz; ohne Eintrag bleibt der Schlüssel"
    );
    assert!(matches!(remap_keys("ohne Verweis", &map), Cow::Borrowed(_)), "unveränderter Text bleibt geliehen");
    assert!(
        matches!(remap_keys(&text, &HashMap::from([(1, 1)])), Cow::Borrowed(_)),
        "gleiche Schlüssel: nichts zu tun"
    );
}
