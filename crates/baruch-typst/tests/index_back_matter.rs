// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 31b: projection (scripture references, first mention) and back matter (`baruch_typst::index`) as text.
//! Whether Typst sets the back matter and which pages it names is measured by
//! `baruch-render/tests/index_measure.rs`. Texts freely invented.

use std::borrow::Cow;

use baruch_typst::index::*;
use baruch_typst::typeset::{GlossaryPosition, IndexPages, TypesetSettings};

fn with(settings: TypesetSettings, glossary: Vec<GlossaryEntry>) -> BackMatter {
    BackMatter { settings, registers: builtin_registers(), glossary }
}

fn pages(registers: &[&str]) -> TypesetSettings {
    TypesetSettings {
        index_pages: Some(IndexPages { registers: registers.iter().map(|s| s.to_string()).collect(), columns: 2 }),
        ..TypesetSettings::default()
    }
}

fn entry(uuid: &str, term: &str, variants: &[&str]) -> GlossaryEntry {
    GlossaryEntry {
        uuid: uuid.into(),
        term: term.into(),
        variants: variants.iter().map(|v| v.to_string()).collect(),
        ..GlossaryEntry::default()
    }
}

#[test]
fn scripture_marks_follow_recognized_passages_in_prose_only() {
    let back = with(pages(&["bibel"]), Vec::new());
    let text = "= Joh 3,16 im Titel\n\
                Wie Joh 3,16 sagt. // Röm 8,28 im Kommentar\n\
                `Ps 23` im Raw, #let s = \"Mt 5,3\" im Code.\n\
                Und Mal 3,20 auch.\n";
    let mut projector = Projector::new(&back);
    let out = projector.project(text).into_owned();
    let marks = find_marks(&out);
    let refs: Vec<(&str, &str, &str)> =
        marks.iter().map(|m| (m.mark.term.as_str(), m.mark.sub.as_str(), m.mark.sort.as_str())).collect();
    assert_eq!(
        refs,
        vec![("Johannes", "3,16", "43.003.016-003.016"), ("Maleachi", "3,20", "39.003.020-003.020")],
        "nur Stellen im Fließtext, Buch als Stichwort, Kapitel/Vers als Unterstichwort, Sortierschlüssel biblisch"
    );
    assert!(marks.iter().all(|m| m.mark.register == SCRIPTURE_REGISTER), "Register bibel");
    assert!(out.contains("Wie Joh 3,16#metadata((register: \"bibel\""), "Marke direkt hinter der Stelle: {out}");
    assert!(out.starts_with("= Joh 3,16 im Titel\n"), "Überschrift unverändert");

    let off = with(pages(&["stichwort"]), Vec::new());
    assert!(
        matches!(Projector::new(&off).project(text), Cow::Borrowed(_)),
        "ohne Register bibel in index_pages keine automatischen Marken"
    );
}

#[test]
fn first_mentions_are_marked_once_per_chapter_with_and_without_glossary_page() {
    let glossary = vec![entry("u-1", "Gnade", &["Huld"]), entry("u-2", "Neuer Bund", &[])];
    let sections =
        ["= Eins\nDie Huld und die Gnade.\n", "== Unter\nNoch einmal Gnade.\n", "= Zwei\ngnade und ein neuer Bund.\n"];
    let run = |settings: TypesetSettings| {
        let back = with(settings, glossary.clone());
        let mut projector = Projector::new(&back);
        sections.iter().map(|s| projector.project(s).into_owned()).collect::<Vec<_>>()
    };
    let plain = run(TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() });
    // Round 21b: without glossary page the invisible note of the projection stands before it (the import recognises
    // it again).
    assert_eq!(
        plain[0], "= Eins\nDie /*baruch-erste-erwaehnung*/#emph[Huld] und die Gnade.\n",
        "Variante zählt als Begriff, danach nicht mehr"
    );
    assert_eq!(plain[1], sections[1], "Abschnitt der Ebene 2 gehört zum selben Kapitel");
    assert_eq!(
        plain[2],
        "= Zwei\n/*baruch-erste-erwaehnung*/#emph[gnade] und ein /*baruch-erste-erwaehnung*/#emph[neuer Bund].\n",
        "neues Kapitel: wieder ausgezeichnet, Groß-/Kleinschreibung egal, Wortgruppe"
    );
    let linked = run(TypesetSettings {
        glossary_first_mention: Some(true),
        glossary_page: Some(GlossaryPosition::AfterIndexes),
        ..TypesetSettings::default()
    });
    assert_eq!(
        linked[0], "= Eins\nDie #link(<baruch-gl-u-1>)[#emph[Huld]] und die Gnade.\n",
        "mit Glossarseite ein Link auf das Label des Eintrags"
    );
    let off =
        run(TypesetSettings { glossary_page: Some(GlossaryPosition::AfterIndexes), ..TypesetSettings::default() });
    assert_eq!(off, sections.map(str::to_owned).to_vec(), "ohne glossary_first_mention keine Auszeichnung");
}

#[test]
fn a_semicolon_guards_characters_typst_would_attach() {
    let back = with(
        TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() },
        vec![entry("u", "Gnade", &[])],
    );
    let out = Projector::new(&back).project("Gnade(n) sind viele.\n").into_owned();
    assert_eq!(
        out, "/*baruch-erste-erwaehnung*/#emph[Gnade];(n) sind viele.\n",
        "`;` beendet den Ausdruck vor der Klammer"
    );
    let out = Projector::new(&back).project("Gnade; so ist es.\n").into_owned();
    assert_eq!(
        out, "/*baruch-erste-erwaehnung*/#emph[Gnade];; so ist es.\n",
        "ein vorhandenes Semikolon bleibt im Satz stehen"
    );
}

#[test]
fn back_matter_is_empty_without_pages_and_starts_with_the_mark_otherwise() {
    let mark = mark_source(&IndexMark::new("stichwort", "Gnade"));
    let body = format!("= Eins\nGnade{mark}.\n");
    assert_eq!(back_matter(&BackMatter::default(), &[&body]), "", "inaktiv: kein Buchende");
    let mention_only =
        with(TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() }, vec![]);
    assert_eq!(back_matter(&mention_only, &[&body]), "", "nur erste Erwähnung: keine Seiten am Buchende");

    let back = with(pages(&["stichwort", "gibt-es-nicht", "personen"]), Vec::new());
    let tail = back_matter(&back, &[&body]);
    assert!(tail.starts_with(&format!("{BACK_MATTER_MARK}\n")), "erste Zeile ist die Marke: {tail}");
    assert!(tail.contains("#heading(level: 1, numbering: none)[#\"Stichwortregister\"]"), "Stichwortregister: {tail}");
    assert!(tail.contains("#heading(level: 1, numbering: none)[#\"Personenregister\"]"), "Personenregister: {tail}");
    assert!(!tail.contains("gibt-es-nicht"), "unbekannte Register-Kennung fällt weg");
    assert!(tail.contains("columns(2, gutter: 1.5em, body)"), "Spaltenzahl aus index_pages.columns");
    assert!(tail.contains("query(<baruch-idx>)"), "Seitenzahlen über query der Marken");
    assert!(tail.contains("counter(page).at(m.location())"), "Seitenzahl wie Fußzeile und Querverweis");
    assert!(tail.contains("#emph(\"Keine Einträge.\")"), "leeres Personenregister sagt das");
    assert!(tail.ends_with('\n'), "endet mit Zeilenende");
}

#[test]
fn data_in_the_back_matter_is_escaped() {
    let tricky = IndexMark::new("stichwort", "Zitat \"mit\" \\ und ] #x");
    let body = format!("Wort{}\n", mark_source(&tricky));
    let gloss = vec![GlossaryEntry {
        uuid: "nicht gültig!".into(),
        term: "Begriff \"A\"".into(),
        long: "Erster Absatz.\n\nZweiter ]Absatz[.".into(),
        ..GlossaryEntry::default()
    }];
    let back =
        with(TypesetSettings { glossary_page: Some(GlossaryPosition::BeforeIndexes), ..pages(&["stichwort"]) }, gloss);
    let tail = back_matter(&back, &[&body]);
    assert!(tail.contains(r#""Zitat \"mit\" \\ und ] #x""#), "Stichwort als maskiertes String-Literal: {tail}");
    assert!(tail.contains(r#""Begriff \"A\"""#), "Begriff maskiert");
    assert!(tail.contains(r#"("Erster Absatz.", "Zweiter ]Absatz[.")"#), "Absätze der Erklärung als Liste");
    assert!(!tail.contains("<baruch-gl-nicht"), "ungültige UUID ergibt kein Label");
    let glossary_at = tail.find("[#\"Glossar\"]").expect("Glossarseite");
    let index_at = tail.find("[#\"Stichwortregister\"]").expect("Register");
    assert!(glossary_at < index_at, "Glossar vor den Registern (before_indexes)");
}

#[test]
fn back_matter_is_the_same_for_the_same_input() {
    let back = with(pages(&["stichwort", "bibel"]), vec![entry("u", "Gnade", &[])]);
    let body = "= Eins\nJoh 3,16 und Gnade.\n";
    let mut p1 = Projector::new(&back);
    let mut p2 = Projector::new(&back);
    let (a, b) = (p1.project(body).into_owned(), p2.project(body).into_owned());
    assert_eq!(a, b, "Projektion deterministisch");
    assert_eq!(back_matter(&back, &[&a]), back_matter(&back, &[&b]), "Buchende deterministisch");
}

/// Integration R31, Round 21b: the import takes a **generated** back matter and the additions of the projection of
/// an export out again; a marker line by hand cuts off nothing.
#[test]
fn an_exported_back_matter_and_the_projection_can_be_stripped_for_the_import() {
    use baruch_typst::index::{has_back_matter_mark, strip_back_matter, strip_first_mentions, strip_projection};
    let text = "= Kap\n\nDas Feuer brennt.\n";
    assert_eq!(strip_back_matter(text), (text, false), "ohne Buchende unverändert");
    let back = with(pages(&["stichwort"]), vec![]);
    let exported = format!("{text}{}", back_matter(&back, &[text]));
    assert!(exported.contains(baruch_typst::index::BACK_MATTER_MARK), "Testvoraussetzung: erzeugtes Buchende");
    assert_eq!(strip_back_matter(&exported), (text, true), "erzeugtes Buchende fällt ab der Kennzeile weg");
    let by_hand = format!("{text}{}\n#let x = 1\nMein Nachwort.\n", baruch_typst::index::BACK_MATTER_MARK);
    assert_eq!(
        strip_back_matter(&by_hand),
        (by_hand.as_str(), false),
        "eine Kennzeile ohne die erzeugten Hilfsfunktionen dahinter schneidet nichts ab (Nachwort bliebe sonst weg)"
    );
    assert!(has_back_matter_mark(&by_hand), "der Aufrufer erkennt die Kennzeile, um sie zu melden");
    let projected = "Das #link(<baruch-gl-ab-12>)[#emph[Feuer]];(hell) brennt, Joh 1,5#metadata((register: \"bibel\", \
                     term: \"Johannes\", sub: \"1,5\", sort: \"43-001-005\"))<baruch-idx> und #emph[Wind].";
    assert_eq!(
        strip_projection(projected),
        "Das Feuer(hell) brennt, Joh 1,5 und #emph[Wind].",
        "Link samt Schutz-Semikolon und automatische Marke fallen weg; Hervorhebung ohne Vermerk bleibt"
    );
    let own = "Gnade#metadata((register: \"stichwort\", term: \"Gnade\"))<baruch-idx>";
    assert_eq!(strip_projection(own), own, "eigene Marken bleiben");
    let plain = "Die /*baruch-erste-erwaehnung*/#emph[Huld];(n) und #emph[Gnade].";
    assert_eq!(
        strip_first_mentions(plain),
        "Die Huld(n) und #emph[Gnade].",
        "vermerkte erste Erwähnung samt Schutz-Semikolon fällt weg, eigene Hervorhebung bleibt"
    );
    assert_eq!(strip_projection(plain), strip_first_mentions(plain), "strip_projection nimmt den Vermerk ebenso");
}

/// Round 21b: projection without glossary page → import → same text as before the projection.
#[test]
fn a_first_mention_without_glossary_page_round_trips_through_the_projection() {
    let back = with(
        TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() },
        vec![entry("u", "Gnade", &[])],
    );
    for body in ["= Eins\nGnade(n) und #emph[Gnade].\n", "= Zwei\nDie Gnade; so ist es.\n", "= Drei\nGnade.feld\n"] {
        let projected = Projector::new(&back).project(body).into_owned();
        assert_ne!(projected, body, "Testvoraussetzung: die Projektion zeichnet aus");
        assert_eq!(
            baruch_typst::index::strip_first_mentions(&projected),
            body,
            "der Import gibt den ursprünglichen Text zurück (auch mit Schutz-Semikolon)"
        );
    }
}
