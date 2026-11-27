// SPDX-License-Identifier: AGPL-3.0-or-later
//! Check wording: find quotations and assign them to a reference, compare normalised, word diff, versification
//! KJV. All texts freely invented.

use baruch_scripture::versification::{KJV, versification};
use baruch_scripture::wording::{QuoteSpan, compare, diff_line, inline_quotes, quote_for};
use baruch_scripture::{books, scan};

fn quote_of(text: &str) -> Option<String> {
    let refs = scan(text);
    let quotes = inline_quotes(text);
    quote_for(text, &quotes, &refs, 0).map(|q| text[q.start..q.end].to_owned())
}

#[test]
fn a_quote_before_or_after_the_reference_belongs_to_it() {
    assert_eq!(
        quote_of("Er schrieb: „Der Kiesel lag am Bach“ (Ps 23,1)."),
        Some("Der Kiesel lag am Bach".into()),
        "davor"
    );
    assert_eq!(quote_of("Ps 23,1: »Der Kiesel lag am Bach.«"), Some("Der Kiesel lag am Bach.".into()), "danach");
    assert_eq!(
        quote_of("„Erstes“ und viel Text dazwischen, der nicht zum Zitat gehört (Ps 23,1)."),
        None,
        "zu weit weg, mit Wörtern dazwischen"
    );
    assert_eq!(quote_of("Ein Satz ohne Zitat (Ps 23,1)."), None, "kein Zitat");
}

#[test]
fn a_reference_list_after_a_quote_shares_it() {
    let text = "„Der Kiesel lag“ (Ps 23,1; Joh 3,16).";
    let refs = scan(text);
    assert_eq!(refs.len(), 2, "zwei Stellen: {refs:?}");
    let quotes = inline_quotes(text);
    assert!(quote_for(text, &quotes, &refs, 1).is_some(), "zweite Stelle der Liste hat dasselbe Zitat");
}

#[test]
fn a_block_quote_with_the_reference_in_its_attribution_is_found() {
    let text = "#quote(block: true, attribution: [Ps 23,1])[Der Kiesel lag am Bach.]";
    let refs = scan(text);
    let open = text.find(")[").unwrap() + 2;
    let block = QuoteSpan { start: open, end: text.len() - 1, outer: (0, text.len()), block: true };
    let q = quote_for(text, &[block], &refs, 0).expect("Blockzitat gefunden");
    assert_eq!(&text[q.start..q.end], "Der Kiesel lag am Bach.", "Inhalt des Blockzitats");
}

#[test]
fn comparison_ignores_case_punctuation_hyphens_and_typography() {
    let bible = "Der Kiesel lag am Bach, und die Amsel sang.";
    assert!(compare("der kiesel lag am bach und die amsel sang", bible).matches, "Groß/Klein und Satzzeichen");
    assert!(compare("Der Kie-\nsel lag am Bach – und die Amsel sang!", bible).matches, "Trennung und Gedankenstrich");
    assert!(compare("Der _Kiesel_ lag am~Bach, und die Amsel sang.", bible).matches, "Typst-Auszeichnung");
    let part = compare("die Amsel sang", bible);
    assert!(part.matches && part.partial, "Teilzitat stimmt und ist als Teil markiert");
    assert!(compare("Der Kiesel … die Amsel sang", bible).matches, "Auslassung mit …");
    assert!(!compare("die Amsel lag", bible).matches, "andere Folge stimmt nicht");
}

#[test]
fn a_differing_quote_has_a_word_diff_limited_to_the_quoted_part() {
    let bible = "Der Kiesel lag am Bach, und die Amsel sang.";
    let c = compare("Der Stein lag am Bach", bible);
    assert!(!c.matches, "weicht ab");
    assert_eq!(diff_line(&c.diff), "Der [−Kiesel] [+Stein] lag am Bach", "nur der zitierte Bereich: {:?}", c.diff);
}

#[test]
fn the_kjv_counting_has_the_english_numbers() {
    let k = versification(KJV).expect("KJV bekannt");
    assert_eq!(k.total_verses(), 31102, "31102 Verse (englische Zählung)");
    assert_eq!(k.chapters_of("Mal").map(<[u16]>::len), Some(4), "Maleachi hat 4 Kapitel");
    assert_eq!(k.chapters_of("Joel").map(<[u16]>::len), Some(3), "Joel hat 3 Kapitel");
    assert_eq!(k.verses_in("Ps", 51), Some(19), "Ps 51 ohne Überschrift als Vers");
    let ids: Vec<&str> = k.books().map(|(id, _)| id).collect();
    let ours: Vec<&str> = books().iter().map(|b| b.id).collect();
    assert_eq!(ids, ours, "dieselben 66 Bücher in derselben Reihenfolge");
}

#[test]
fn typst_characters_in_the_bible_text_are_plain_text() {
    // The sermon text is escaped in the Typst source text ("\#1"), the Bible text is plain text ("#1"): both must
    // yield the same words, otherwise Baruch would itself report a deviation for the inserted sermon text.
    let quote = "Lobt \\#1 den \\*Hirten\\* \\_hier\\_ für \\$5 an \\@tor \\<weg\\> mit \\\\ Stab\\~Ende.";
    let bible = "Lobt #1 den *Hirten* _hier_ für $5 an @tor <weg> mit \\ Stab~Ende.";
    let c = compare(quote, bible);
    assert!(c.matches && !c.partial, "maskiertes Zitat und Klartext stimmen überein: {:?}", c.diff);
}

#[test]
fn nested_quotes_count_as_one_and_quotes_may_span_paragraphs() {
    assert_eq!(
        quote_of("„Er rief: ‚Komm‘ und »geh«.“ (Joh 3,16)").as_deref(),
        Some("Er rief: ‚Komm‘ und »geh«."),
        "innere Anführungszeichen anderer Art gehören zum äußeren Zitat"
    );
    assert_eq!(
        quote_of("„Erster Absatz.\n\n„Zweiter Absatz.“ (Joh 3,16)").as_deref(),
        Some("Erster Absatz.\n\n„Zweiter Absatz."),
        "deutscher Satz: am Anfang des nächsten Absatzes wiederholtes Zeichen setzt das Zitat fort"
    );
    assert_eq!(
        quote_of("„Erster Absatz\n\nzweiter Absatz.“ (Joh 3,16)").as_deref(),
        Some("Erster Absatz\n\nzweiter Absatz."),
        "auch ohne wiederholtes Zeichen über die Leerzeile bis zum Schlusszeichen"
    );
    let spans = inline_quotes("„Offen.\n\nDann „Zu“ hier.");
    assert_eq!(
        spans.len(),
        1,
        "ein weiteres öffnendes Zeichen mitten im Absatz: das erste war nie geschlossen: {spans:?}"
    );
}

#[test]
fn a_reference_in_a_new_paragraph_takes_the_quote_after_it_not_the_one_of_the_previous_paragraph() {
    // Integrator Round 17: „Psalm 139, Vers 23: „…““ in the next paragraph previously got the quotation of the
    // previous paragraph (which belongs to Joh 3,16), because whitespace across the empty line counted as a link.
    let text = "„Der Fährmann sprach.“ (Joh 3,16)\n\nPsalm 139, Vers 23: „Prüfe den Steg.“";
    let refs = scan(text);
    let quotes = inline_quotes(text);
    let of = |i: usize| quote_for(text, &quotes, &refs, i).map(|q| text[q.start..q.end].to_owned());
    assert_eq!(of(0).as_deref(), Some("Der Fährmann sprach."), "Joh 3,16 behält sein Zitat davor");
    assert_eq!(of(1).as_deref(), Some("Prüfe den Steg."), "Ps 139,23 bekommt das Zitat danach im selben Absatz");
    let alone = "„Der Fährmann sprach.“\n\n(Joh 3,16)";
    let refs = scan(alone);
    let quotes = inline_quotes(alone);
    assert_eq!(
        quote_for(alone, &quotes, &refs, 0).map(|q| alone[q.start..q.end].to_owned()).as_deref(),
        Some("Der Fährmann sprach."),
        "ohne Zitat danach gilt das Zitat davor weiter, auch über die Leerzeile"
    );
}
