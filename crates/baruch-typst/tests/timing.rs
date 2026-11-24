// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zeitwächter (Runde 16b, Schnittstelle R16-S1): Pflicht-Beispiele des Vertrags und die Zählregeln.
//! Alle Texte sind frei erfunden.

use baruch_typst::timing::{
    DEFAULT_WORDS_PER_MINUTE, MAX_TARGET_MINUTES, MAX_WORDS_PER_MINUTE, MIN_WORDS_PER_MINUTE, clock, speaking_seconds,
    spoken_words,
};

#[test]
fn the_contract_examples_count_exactly_the_spoken_words() {
    assert_eq!(
        spoken_words("= Einleitung\n\nGott ist *treu*."),
        3,
        "die Überschriftszeile zählt nicht, die Sternchen fallen weg: „Gott ist treu.“"
    );
    assert_eq!(
        spoken_words("#quote(block: true)[Im Anfang war das Wort.] <zitat>"),
        5,
        "Name und Argumente von #quote fallen weg, der Inhaltsblock wird gesprochen, das Label nicht"
    );
    assert_eq!(
        spoken_words("Text#footnote[Quelle: Buch]. Mehr // Notiz\n"),
        2,
        "die Fußnote wird nicht gesprochen und der Kommentar nach Leerraum zählt nicht: „Text. Mehr“"
    );
    assert_eq!(
        spoken_words("#set text(size: 12pt)\nHallo Welt"),
        2,
        "eine #set-Zeile fällt ganz weg, übrig bleibt „Hallo Welt“"
    );
    assert_eq!(spoken_words("$a^2$ und `code` hier"), 2, "Mathe und Raw zählen nicht, übrig bleibt „und hier“");
}

#[test]
fn the_contract_examples_give_the_expected_seconds_and_clocks() {
    assert_eq!(speaking_seconds(110, 110), 60, "110 Wörter bei 110 Wörtern je Minute sind genau eine Minute");
    assert_eq!(speaking_seconds(1, 110), 1, "60/110 s wird aufgerundet: ein Wort braucht eine Sekunde");
    assert_eq!(speaking_seconds(0, 110), 0, "kein Wort braucht keine Zeit");
    assert_eq!(clock(45), "0:45", "unter einer Minute: Minuten 0 und zweistellige Sekunden");
    assert_eq!(clock(725), "12:05", "12 Minuten 5 Sekunden, Sekunden zweistellig");
    assert_eq!(clock(3723), "1:02:03", "ab einer Stunde h:mm:ss mit zweistelligen Minuten");
}

#[test]
fn the_constants_are_the_ones_of_the_contract() {
    assert_eq!(DEFAULT_WORDS_PER_MINUTE, 110, "Vorgabe des Vertrags: 110 Wörter je Minute");
    assert_eq!(
        (MIN_WORDS_PER_MINUTE, MAX_WORDS_PER_MINUTE),
        (60, 250),
        "Grenzen der Sprechgeschwindigkeit wie in Migration 0039"
    );
    assert_eq!(MAX_TARGET_MINUTES, 240, "längste Zielzeit 240 Minuten = 14400 s wie in Migration 0039");
}

#[test]
fn words_per_minute_outside_the_limits_are_clamped() {
    assert_eq!(speaking_seconds(60, 10), 60, "10 Wörter je Minute wird auf 60 geklemmt: 60 Wörter = 60 s");
    assert_eq!(speaking_seconds(250, 1000), 60, "1000 Wörter je Minute wird auf 250 geklemmt: 250 Wörter = 60 s");
    assert_eq!(speaking_seconds(60, 0), 60, "0 Wörter je Minute teilt nicht durch null, sondern klemmt auf 60");
}

#[test]
fn comments_count_only_where_typst_sees_them() {
    assert_eq!(
        spoken_words("Siehe https://beispiel.invalid/seite heute"),
        3,
        "// nach einem Doppelpunkt ist kein Kommentar, die Adresse ist ein Wort"
    );
    assert_eq!(
        spoken_words("Vorher /* ein\nlanger\nKommentar */ nachher"),
        2,
        "ein mehrzeiliger Blockkommentar zählt nicht"
    );
    assert_eq!(spoken_words("// nur eine Notiz\nEin Satz."), 2, "ein Kommentar am Zeilenanfang zählt nicht");
}

#[test]
fn headings_and_part_markers_do_not_count() {
    let text = "#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>\n  == Unterpunkt\nNur dies.";
    assert_eq!(spoken_words(text), 2, "Teil-Markierung und eingerückte Überschrift fallen weg: „Nur dies.“");
    assert_eq!(spoken_words("==Kein Leerzeichen"), 2, "ohne Leerzeichen nach = ist es keine Überschrift");
}

#[test]
fn code_lines_fall_away_even_when_their_brackets_span_lines() {
    let text = "#let gruss(name) = [\n  Hallo #name\n]\n#show heading: it => {\n  it\n}\nDanach ein Satz.";
    assert_eq!(spoken_words(text), 3, "#let und #show samt mehrzeiligen Klammern fallen weg: „Danach ein Satz.“");
    assert_eq!(
        spoken_words("#import \"vorlage.typ\": *\n#include \"teil.typ\"\nEins zwei"),
        2,
        "#import- und #include-Zeilen fallen ganz weg"
    );
}

#[test]
fn code_arguments_fall_away_but_spoken_content_counts() {
    assert_eq!(
        spoken_words("#text(fill: red, \"a ] b\")[rote Worte] bleiben"),
        3,
        "Argumente mit Zeichenkette samt ] fallen weg; Inhaltsblock und Folgetext zählen"
    );
    assert_eq!(spoken_words("#emph[sehr] gut"), 2, "#emph: der Inhalt wird gesprochen");
    assert_eq!(spoken_words("#arbeit[intern zu prüfen] Gnade"), 1, "#arbeit wird nicht gesprochen");
    assert_eq!(spoken_words("Wert #wert.feld und mehr"), 3, "#name.feld fällt ganz weg");
}

#[test]
fn labels_references_and_markup_signs_fall_away() {
    assert_eq!(spoken_words("Wie in @kapitel-eins gesagt <hier>"), 3, "Verweis und Label fallen weg");
    assert_eq!(spoken_words("schreib an a@b.invalid"), 3, "ein @ mitten im Wort ist kein Verweis");
    assert_eq!(spoken_words("- erstens\n+ zweitens\n/ Begriff: drittens"), 4, "Listenzeichen fallen weg");
    assert_eq!(spoken_words("Joh~3,16 und Mt 5"), 5, "~ trennt wie ein Leerzeichen: Joh, 3,16, und, Mt, 5");
    assert_eq!(spoken_words("Zeile\\\nweiter"), 2, "\\ als Zeilenumbruch trennt zwei Wörter");
    assert_eq!(
        spoken_words("\\* nicht fett \\#keincode"),
        3,
        "Maskierungen: \\* allein ist kein Wort, \\#keincode ist eins"
    );
    assert_eq!(spoken_words("Gott-Vertrauen trägt"), 2, "ein Bindestrichwort ist ein Wort");
    assert_eq!(spoken_words("Ein Satz \u{2013} mit Strich"), 4, "ein freistehender Gedankenstrich ist kein Wort");
}

#[test]
fn windows_line_ends_count_like_unix_line_ends() {
    assert_eq!(
        spoken_words("= Titel\r\n\r\nZwei Wörter\r\n"),
        spoken_words("= Titel\n\nZwei Wörter\n"),
        "CRLF wird zu LF, die Überschrift wird in beiden Fällen erkannt"
    );
    assert_eq!(spoken_words("= Titel\r\n\r\nZwei Wörter\r\n"), 2, "nur der Text unter der Überschrift zählt");
}

#[test]
fn an_unclosed_bracket_or_raw_does_not_panic_and_ends_the_count() {
    assert_eq!(spoken_words("Am Ende `offen"), 2, "offenes Raw reicht bis zum Textende");
    assert_eq!(spoken_words("Ein #f(offen"), 1, "offene Argumente reichen bis zum Textende");
    assert_eq!(spoken_words("#quote[offen und"), 2, "ein offener Inhaltsblock wird bis zum Ende gesprochen");
    assert_eq!(spoken_words(""), 0, "leerer Text hat keine Wörter");
}
