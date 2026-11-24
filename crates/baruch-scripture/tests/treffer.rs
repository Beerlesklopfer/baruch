// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 19c (treffer): Reste der Stellen in `baruch-scripture` – „Mal“ als Buch oder Zahlwort, kurze
//! Einleitungswörter zwischen Stelle und Zitat, Wortlaut ohne Leerraum (Trennreste im Modul), Unicode-Normalform
//! und die Schätzung der Wortfragmente eines Moduls. Alle Texte frei erfunden.

use baruch_scripture::scan;
use baruch_scripture::wording::{compare, fragment_survey, inline_quotes, quote_for};

fn labels(text: &str) -> Vec<String> {
    scan(text).into_iter().filter_map(|f| f.normalized).collect()
}

fn quote_of(text: &str) -> Option<String> {
    let refs = scan(text);
    let quotes = inline_quotes(text);
    quote_for(text, &quotes, &refs, 0).map(|q| text[q.start..q.end].to_owned())
}

// ---- „Mal“ ---------------------------------------------------------------------------------------------

#[test]
fn mal_followed_by_chapter_and_verse_is_malachi_and_keeps_the_reference_before_it() {
    assert_eq!(
        labels("Er las Joh 3,16 Mal 3,20 vor."),
        ["Joh 3,16", "Mal 3,20"],
        "Begründung: Vor Runde 19 galt „Mal“ hinter „Joh 3,16“ als Zahlwort und Joh 3,16 entfiel still; mit Kapitel \
         und Vers dahinter ist „Mal“ Maleachi, beide Stellen bleiben"
    );
}

#[test]
fn mal_without_chapter_and_verse_stays_a_count_word() {
    assert!(
        labels("Er hat es Johannes 3 Mal gesagt.").is_empty(),
        "Begründung: „3 Mal“ ohne Folgezahl ist eine Anzahl, „Johannes 3“ also keine Stelle (wie seit Runde 17b)"
    );
    assert!(
        labels("Er war Johannes 3 Mal 2 Tage voraus.").is_empty(),
        "Begründung: eine bloße Zahl hinter „Mal“ („2 Tage“) ist keine Kapitel-/Versangabe, „Mal“ bleibt Zahlwort"
    );
    assert_eq!(
        labels("Vgl. Joh 3,16, 18 Mal."),
        ["Joh 3,16"],
        "Begründung: „, 18 Mal.“ bleibt eine Anzahl und wird kein Vers 18 (Satzpunkt statt Kapitel und Vers)"
    );
}

// ---- Zitat-Bindeglied ----------------------------------------------------------------------------------

#[test]
fn short_introducing_words_between_reference_and_quote_are_allowed() {
    assert_eq!(
        quote_of("Wie Lk 15,6 erzählt: „Freut euch mit mir über das Lamm.“"),
        Some("Freut euch mit mir über das Lamm.".to_owned()),
        "Begründung: „erzählt:“ ist ein kurzes Einleitungswort mit Doppelpunkt; vor Runde 19 ergab das „ohne Zitat“"
    );
    assert_eq!(
        quote_of("In Ps 23,1 heißt es: „Der Kiesel lag am Bach.“"),
        Some("Der Kiesel lag am Bach.".to_owned()),
        "Begründung: zwei Einleitungswörter („heißt es:“) bleiben unter der Grenze von drei Wörtern"
    );
    assert_eq!(
        quote_of("Joh 3,16 sagt: »Die Amsel sang am Morgen.«"),
        Some("Die Amsel sang am Morgen.".to_owned()),
        "Begründung: „sagt:“ steht in der Liste; französische Anführungszeichen zählen wie deutsche"
    );
}

#[test]
fn long_text_between_reference_and_quote_still_means_no_quote() {
    assert_eq!(
        quote_of("Lk 15,6 erzählt, wie der Hirte das Lamm über viele Hügel nach Hause trägt: „Freut euch mit mir.“"),
        None,
        "Begründung: ein ganzer Zwischensatz ist kein Bindeglied; das Zitat gehört nicht sicher zur Stelle"
    );
    assert_eq!(
        quote_of("Lk 15,6 erzählt „Freut euch mit mir.“"),
        None,
        "Begründung: ohne Doppelpunkt ist „erzählt“ kein Bindeglied (das Zitat könnte Gegenstand des Satzes sein)"
    );
    assert_eq!(
        quote_of("Lk 15,6 Kiesel: „Freut euch mit mir.“"),
        None,
        "Begründung: nur Wörter aus der Liste der Einleitungswörter zählen, nicht beliebige"
    );
}

// ---- Wortlaut ohne Leerraum ----------------------------------------------------------------------------

#[test]
fn a_quote_that_differs_only_in_spacing_matches_with_the_spacing_flag() {
    let bible = "Die ses Wort gilt al len Men schen, die am Ufer war ten.";
    let c = compare("Dieses Wort gilt allen Menschen, die am Ufer warten.", bible);
    assert!(c.matches, "Begründung: nur der Leerraum im Modul weicht ab (Trennreste), der Wortlaut stimmt: {c:?}");
    assert!(c.spacing, "Begründung: der Treffer gelang nur ohne Leerraum, das zeigt `spacing`: {c:?}");
    assert!(!c.partial, "Begründung: das Zitat umfasst den ganzen Vers: {c:?}");
    let part = compare("allen Menschen", bible);
    assert!(part.matches && part.spacing && part.partial, "Begründung: Teilzitat über Trennreste: {part:?}");
}

#[test]
fn a_real_difference_stays_a_difference_even_without_spacing() {
    let bible = "Die ses Wort gilt al len Men schen.";
    let c = compare("Dieses Wort gilt allen Völkern.", bible);
    assert!(!c.matches, "Begründung: „Völkern“ statt „Menschen“ ist eine echte Abweichung: {c:?}");
    assert!(!c.spacing, "Begründung: kein Treffer, also auch kein Leerraum-Treffer: {c:?}");
    assert!(!c.diff.is_empty(), "Begründung: der Wort-Diff zeigt die Abweichung wie bisher: {c:?}");
    assert!(
        !compare("ist", "Christus lehrt.").matches,
        "Begründung: ohne Leerraum zählt nur ein Treffer von Wortgrenze zu Wortgrenze, nicht mitten in einem Wort"
    );
}

/// Runde 23a (Gutachten R19): Der Vergleich ohne Leerraum duldet nur zusätzliche Wortgrenzen des Moduls.
#[test]
fn a_quote_with_its_own_word_boundaries_does_not_match_other_words_of_the_module() {
    let c = compare("die Bein", "Diebe in der Nacht.");
    assert!(
        !c.matches && !c.spacing,
        "Begründung: „die Bein“ hat eine Wortgrenze, die das Modul („Diebe in“) nicht hat – andere Wörter, kein \
         Leerraum-Treffer (vorher: „stimmt (Leerraum im Modul)“): {c:?}"
    );
    let c = compare("am Ende", "Amen, de r Herr kommt.");
    assert!(!c.matches, "Begründung: „am Ende“ steckt nur über fremde Wortgrenzen in „Amen de“: {c:?}");
    let ok = compare("Diebe in der Nacht", "Die be in der Na cht.");
    assert!(
        ok.matches && ok.spacing,
        "Begründung: hier hat nur das Modul zusätzliche Grenzen (Trennreste); alle Grenzen des Zitats gibt es auch \
         im Modul: {ok:?}"
    );
}

#[test]
fn a_clean_match_does_not_set_the_spacing_flag() {
    let c = compare("Der Kiesel lag am Bach", "Der Kiesel lag am Bach, und die Amsel sang.");
    assert!(c.matches && !c.spacing, "Begründung: gleiche Wörter brauchen keinen Vergleich ohne Leerraum: {c:?}");
}

#[test]
fn decomposed_umlauts_compare_like_composed_ones() {
    let decomposed = "U\u{308}ber den Hu\u{308}gel ging er.";
    let c = compare("Über den Hügel ging er.", decomposed);
    assert!(c.matches, "Begründung: NFC vor dem Zerlegen – „u“ + U+0308 ist dasselbe Wort wie „ü“: {c:?}");
    assert!(!c.spacing, "Begründung: der Treffer braucht keinen Vergleich ohne Leerraum: {c:?}");
}

// ---- Wortfragmente eines Moduls ------------------------------------------------------------------------

#[test]
fn a_text_with_many_remnants_is_fragmented_and_a_clean_one_is_not() {
    let remnants = [
        "Die ses Wort gilt allen Menschen am Ufer.",
        "Alle Men schen hörten dieses Wort am Abend.",
        "Er lehrte die Menschen und sagte die ses Lied.",
        "Am Ufer warteten die Fischer auf das Boot.",
        "Die Fischer war ten am Ufer bis zum Morgen.",
    ];
    let survey = fragment_survey(remnants);
    assert!(
        survey.suspects >= 3,
        "Begründung: „Die ses“, „Men schen“ und „die ses“ stehen neben „dieses“ und „Menschen“ – drei Fälle, deren \
         Zusammenschreibung sonst im Text vorkommt („war ten“ zählt nicht, „warten“ kommt nicht vor): {survey:?}"
    );
    assert!(survey.fragmented(), "Begründung: viele Fälle bei wenigen Wortpaaren sind auffällig: {survey:?}");
    assert!(
        survey.examples.iter().any(|e| e == "Men schen"),
        "Begründung: Beispiele nennen das Paar, wie es im Modul steht: {survey:?}"
    );

    let clean = [
        "Dieses Wort gilt allen Menschen am Ufer.",
        "Alle Menschen hörten dieses Wort am Abend.",
        "Er ging hin aus dem Haus und sah da mit Freude den Morgen.",
    ];
    let survey = fragment_survey(clean);
    assert_eq!(
        survey.suspects, 0,
        "Begründung: ohne Trennreste gibt es keinen Fall; „hin aus“ und „da mit“ sind häufige kurze Wörter und zählen \
         nie: {survey:?}"
    );
    assert!(!survey.fragmented(), "Begründung: ein sauberes Modul bekommt keinen Hinweis: {survey:?}");
}
