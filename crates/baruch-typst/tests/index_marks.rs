// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31b: Registermarken finden, setzen und entfernen (`baruch_typst::index`). Texte frei erfunden.

use baruch_typst::index::*;

fn gnade() -> IndexMark {
    IndexMark::new("stichwort", "Gnade")
}

const MARK: &str = r#"#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>"#;

#[test]
fn mark_word_sets_the_canonical_mark_directly_behind_the_word() {
    let text = "= Kapitel\n\nDie Gnade trägt.\n";
    let (out, n) = mark_word(text, "Gnade", Occurrence::First, &gnade()).expect("Wort steht im Text");
    assert_eq!(n, 1, "eine Marke gesetzt");
    assert_eq!(out, format!("= Kapitel\n\nDie Gnade{MARK} trägt.\n"), "Marke direkt hinter dem Wort, kein Leerzeichen");
    let found = find_marks(&out);
    assert_eq!(found.len(), 1, "find_marks findet die gesetzte Marke");
    assert_eq!(&out[found[0].span.0..found[0].span.1], MARK, "Bereich = ganze Marke");
    assert_eq!(found[0].mark, gnade(), "Rundreise mark_source → parse_mark");
    assert_eq!(remove_mark(&out, found[0].span).as_deref(), Ok(text), "Entfernen stellt den Text wieder her");
}

#[test]
fn headings_code_comments_raw_math_and_other_marks_are_skipped() {
    let text = "= Gnade im Titel\n\
                #heading(level: 1, numbering: none)[Gnade] <baruch-part>\n\
                // Gnade im Kommentar\n\
                /* Gnade\n im Block */\n\
                `Gnade` und ```\nGnade\n```\n\
                $Gnade$ #let gnade = \"Gnade\"\n\
                #emph[Die Gnade] bleibt.\n";
    let (out, n) = mark_word(text, "Gnade", Occurrence::All, &gnade()).expect("eine Fundstelle im Fließtext");
    assert_eq!(n, 1, "nur die Gnade im Inhaltsblock von #emph ist Fließtext: {out}");
    assert!(out.contains(&format!("#emph[Die Gnade{MARK}] bleibt.")), "Marke im Inhaltsblock: {out}");
    assert!(out.starts_with("= Gnade im Titel\n"), "Überschriftszeile unverändert: {out}");

    let only_hidden = "= Gnade\n// Gnade\n`Gnade`\n";
    assert_eq!(
        mark_word(only_hidden, "Gnade", Occurrence::First, &gnade()),
        Err(IndexError::WordNotFound("Gnade".into())),
        "nur in Überschrift, Kommentar und Raw: nicht gefunden"
    );

    // Eine Marke im Text ist keine Fundstelle (ihr Stichwort „Gnade“ steht in einem String); eine andere Marke hinter
    // dem Wort verhindert die neue nicht.
    let marked = format!("Gnade{MARK} und Huld.\n");
    let huld = IndexMark::new("stichwort", "Huld");
    let (again, n) = mark_word(&marked, "Gnade", Occurrence::All, &huld).expect("das Wort selbst");
    assert_eq!(n, 1, "nur das Wort, nicht der Text in der Marke");
    assert!(
        again.starts_with(&format!("Gnade#metadata((register: \"stichwort\", term: \"Huld\"))<baruch-idx>{MARK} und")),
        "neue Marke direkt hinter dem Wort, die vorhandene bleibt dahinter: {again}"
    );
}

#[test]
fn the_same_mark_is_never_set_twice() {
    // Befund nach R31: Zweimal „In Register aufnehmen“ bzw. ein Registervorschlag für ein schon markiertes Wort setzte
    // eine zweite, gleiche Marke (doppelt in Übersicht und index.entries, jede Doppelung eine neue Fassung).
    let text = "Die Gnade trägt. Gnade bleibt.\n";
    let (once, n) = mark_word(text, "Gnade", Occurrence::First, &gnade()).expect("erste Fundstelle");
    assert_eq!(n, 1, "eine Marke");
    assert_eq!(
        mark_word(&once, "Gnade", Occurrence::First, &gnade()),
        Err(IndexError::AlreadyMarked("Gnade".into())),
        "„erste“ ist schon markiert: ehrlicher Fehler statt zweiter Marke (der Aufrufer legt keine Fassung an)"
    );
    assert_eq!(find_marks(&once).len(), 1, "weiter genau eine Marke");
    // Die zweite Fundstelle lässt sich gezielt markieren; „alle“ setzt danach nur, was fehlt.
    let (both, n) = mark_word(&once, "Gnade", Occurrence::All, &gnade()).expect("die zweite fehlt noch");
    assert_eq!(n, 1, "„alle“ setzt nur die fehlende Marke");
    assert_eq!(both, format!("Die Gnade{MARK} trägt. Gnade{MARK} bleibt.\n"), "je Fundstelle genau eine Marke");
    assert_eq!(
        mark_word(&both, "Gnade", Occurrence::All, &gnade()),
        Err(IndexError::AlreadyMarked("Gnade".into())),
        "nichts fehlt mehr"
    );
    assert_eq!(
        mark_word(&both, "Gnade", Occurrence::Nth(2), &gnade()),
        Err(IndexError::AlreadyMarked("Gnade".into())),
        "„2.“ ebenso"
    );
    // Auch wenn die gleiche Marke erst hinter einer anderen Marke bzw. hinter einem Label steht, gilt sie als gesetzt.
    let chain = format!("Gnade#metadata((register: \"personen\", term: \"Gnade\"))<baruch-idx>{MARK} hier.\n");
    assert_eq!(
        mark_word(&chain, "Gnade", Occurrence::First, &gnade()),
        Err(IndexError::AlreadyMarked("Gnade".into())),
        "zweite Marke der Kette ist die gleiche"
    );
    let labelled = format!("Gnade<anker>{MARK} hier.\n");
    assert_eq!(
        mark_word(&labelled, "Gnade", Occurrence::First, &gnade()),
        Err(IndexError::AlreadyMarked("Gnade".into())),
        "Marke hinter dem Label"
    );
    // Ein anderes Unterstichwort ist eine andere Marke.
    let mut sub = gnade();
    sub.sub = "Rechtfertigung".into();
    let (more, n) = mark_word(&once, "Gnade", Occurrence::First, &sub).expect("andere Marke");
    assert_eq!((n, find_marks(&more).len()), (1, 2), "andere Marke an derselben Stelle ist erlaubt");
}

#[test]
fn whole_words_and_occurrences() {
    let text = "Gnadenzeit, Gnade, Begnadigung und noch einmal Gnade.\n";
    let (first, _) = mark_word(text, "Gnade", Occurrence::First, &gnade()).expect("erste");
    assert_eq!(first, format!("Gnadenzeit, Gnade{MARK}, Begnadigung und noch einmal Gnade.\n"), "ganze Wörter");
    let (second, _) = mark_word(text, "Gnade", Occurrence::Nth(2), &gnade()).expect("zweite");
    assert!(second.ends_with(&format!("einmal Gnade{MARK}.\n")), "zweite Fundstelle: {second}");
    let (_, all) = mark_word(text, "Gnade", Occurrence::All, &gnade()).expect("alle");
    assert_eq!(all, 2, "zwei ganze Wörter „Gnade“");
    assert_eq!(
        mark_word(text, "Gnade", Occurrence::Nth(3), &gnade()),
        Err(IndexError::WordNotFound("Gnade".into())),
        "eine dritte gibt es nicht"
    );
    let phrase = "Der Neue\nBund gilt.\n";
    let (out, _) = mark_word(phrase, "Neue Bund", Occurrence::First, &IndexMark::new("stichwort", "Neuer Bund"))
        .expect("Wortgruppe über einen Zeilenumbruch");
    assert!(out.contains("Bund#metadata(("), "Marke hinter dem letzten Wort der Gruppe: {out}");
    assert!(
        mark_word("Der Neue\n\nBund.\n", "Neue Bund", Occurrence::First, &gnade()).is_err(),
        "keine Wortgruppe über eine Leerzeile (Absatzgrenze)"
    );
}

#[test]
fn invalid_marks_and_empty_words_are_refused() {
    let bad = IndexMark::new("Stichwort", "Gnade");
    assert!(matches!(mark_word("Gnade", "Gnade", Occurrence::First, &bad), Err(IndexError::Invalid(_))), "Kennung");
    assert!(
        matches!(mark_word("Gnade", "  ", Occurrence::First, &gnade()), Err(IndexError::Invalid(_))),
        "leeres Wort"
    );
    assert!(
        matches!(insert_mark_at("Gnade", 5, &bad), Err(IndexError::Invalid(_))),
        "insert_mark_at prüft die Marke ebenso"
    );
    assert!(
        matches!(insert_mark_at("Gnäde", 3, &gnade()), Err(IndexError::Invalid(_))),
        "Stelle mitten in einem Zeichen (ä hat zwei Bytes)"
    );
}

#[test]
fn insert_mark_at_refuses_places_inside_constructs() {
    let text = "= Titel\nText #emph[Wort] und `raw` // Kommentar\nEnde\n";
    let ok = text.find(" und").expect("Stelle");
    let out = insert_mark_at(text, ok, &gnade()).expect("hinter #emph[…] ist Fließtext");
    assert!(out.contains(&format!("#emph[Wort]{MARK} und")), "eingefügt: {out}");
    let refused = [
        (text.find("Titel").expect("Titel"), "in der Überschriftszeile"),
        (0, "am Anfang der Überschriftszeile"),
        (text.find("emph").expect("emph"), "im Funktionsnamen"),
        (text.find("[Wort").expect("Klammer"), "zwischen Name und Inhaltsblock"),
        (text.find("raw`").expect("raw"), "im Raw"),
        (text.find("Kommentar").expect("Kommentar"), "im Kommentar"),
        (text.find("\nEnde").expect("Zeilenende"), "am Ende der Kommentarzeile"),
        (text.find("\nText").expect("Zeilenende"), "am Ende der Überschriftszeile"),
        (text.len() + 1, "hinter dem Text"),
    ];
    for (at, why) in refused {
        assert!(matches!(insert_mark_at(text, at, &gnade()), Err(IndexError::Invalid(_))), "abgelehnt {why} ({at})");
    }
    let marked = format!("A{MARK} B");
    assert!(
        matches!(insert_mark_at(&marked, 5, &gnade()), Err(IndexError::Invalid(_))),
        "nicht in einer anderen Marke"
    );
    let label = "Wort<anker> weiter";
    assert!(
        matches!(insert_mark_at(label, 4, &gnade()), Err(IndexError::Invalid(_))),
        "nicht vor einem Label (es hinge sonst an der Marke)"
    );
    let (behind, _) = mark_word(label, "Wort", Occurrence::First, &gnade()).expect("Wort mit Label");
    assert_eq!(behind, format!("Wort<anker>{MARK} weiter"), "mark_word setzt hinter das Label");
}

#[test]
fn find_marks_skips_marks_in_comments_raw_and_strings() {
    let text =
        format!("A{MARK}\n// B{MARK}\n`C{MARK}`\n#let s = \"{}\"\n#footnote[D{MARK}]\n", MARK.replace('"', "\\\""));
    let found = find_marks(&text);
    assert_eq!(found.len(), 2, "Fließtext und Fußnoten-Inhalt zählen, Kommentar/Raw/String nicht: {found:?}");
    assert!(found[0].span.0 < found[1].span.0, "Textreihenfolge");
    assert!(matches!(remove_mark(&text, (0, 3)), Err(IndexError::Invalid(_))), "kein Markenbereich");
}

#[test]
fn sort_keys_follow_din_5007_1() {
    assert_eq!(sort_key("Ähre"), "ahre", "Ä wie A");
    assert_eq!(sort_key("Öl"), sort_key("ol"), "Ö wie O");
    assert_eq!(sort_key("Übung"), "ubung", "Ü wie U");
    assert_eq!(sort_key("Straße"), "strasse", "ß wie ss");
    assert_eq!(sort_key("Café"), "cafe", "Akzente fallen weg");
    assert_eq!(sort_key("„Zitat“"), "zitat“", "Satzzeichen am Anfang übergangen");
    let mut words = vec!["Zeder", "Ölberg", "Abend", "Ähre", "Ochse", "Ufer", "Übung"];
    words.sort_by_key(|w| sort_key(w));
    assert_eq!(
        words,
        vec!["Abend", "Ähre", "Ochse", "Ölberg", "Übung", "Ufer", "Zeder"],
        "deutsche Reihenfolge: Ä bei A, Ö bei O, Übung (ubung) vor Ufer, nicht hinter Z"
    );
}

#[test]
fn overview_groups_marks_per_register_with_section_keys() {
    let back = BackMatter { registers: builtin_registers(), ..BackMatter::default() };
    let a = format!("Gnade{MARK} und Noomi{}.\n", mark_source(&IndexMark::new("personen", "Noomi")));
    let b = format!(
        "Huld{} und Gnade{MARK}.\n",
        mark_source(&IndexMark { see: "Gnade".into(), ..IndexMark::new("stichwort", "Huld") })
    );
    let views = overview(&back, &[(11, &a), (12, &b), (13, "Joh 3,16 ohne Register")]);
    assert_eq!(views.len(), 3, "alle drei eingebauten Register");
    let keyword = &views[0];
    assert_eq!(keyword.register.slug, "stichwort", "Reihenfolge wie back.registers");
    let terms: Vec<(&str, &[i64], &[String])> =
        keyword.terms.iter().map(|t| (t.term.as_str(), t.keys.as_slice(), t.see.as_slice())).collect();
    assert_eq!(
        terms,
        vec![("Gnade", &[11i64, 12][..], &[][..]), ("Huld", &[][..], &["Gnade".to_owned()][..])],
        "Gnade in zwei Abschnitten, Huld nur „siehe“ ohne Abschnitt"
    );
    assert_eq!(views[1].terms[0].term, "Noomi", "Personenregister");
    assert!(views[2].terms.is_empty(), "Bibelstellen nur, wenn das Bibelstellenregister eingeschaltet ist");
}
