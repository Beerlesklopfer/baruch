// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 19c (treffer) in `baruch-sword`: Lemma-Probe der Lexika (Umlaut-Rückführung, Komposita, Strong-Nummern mit
//! und ohne `StrongsPadding`), Stichprobe eines Moduls und erfundene Module mit Trennresten
//! ([`baruch_sword::fixture::hyphen_remnants`]). Alle Module und Texte frei erfunden, in Temp-Ordnern.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use baruch_scripture::versification::german;
use baruch_scripture::wording::fragment_survey;
use baruch_sword::fixture::{Block, Fixture, Layout, LexLayout, LexiconFixture, hyphen_remnants, write, write_lexicon};
use baruch_sword::reference::{compound_tails, lemma_candidates, strong_like};
use baruch_sword::{open, open_lexicon, scan};

fn root() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sword");
    (dir, root)
}

fn lexicon(root: &std::path::Path, fixture: &LexiconFixture) -> baruch_sword::Lexicon {
    write_lexicon(root, fixture).unwrap();
    let found = scan(&[root.to_path_buf()]);
    open_lexicon(found.find_lexicon(&fixture.name).expect("Testlexikon gefunden")).unwrap()
}

// ---- Kandidaten ----------------------------------------------------------------------------------------

#[test]
fn lemma_candidates_reverse_the_umlaut_after_the_endings() {
    let found = lemma_candidates("Bäume");
    assert_eq!(found[0], "Bäume", "Begründung: das Wort selbst zuerst (kleinste Änderung): {found:?}");
    assert!(found.contains(&"Bäum".to_owned()), "Begründung: Endung -e abgeschnitten: {found:?}");
    assert!(found.contains(&"Baum".to_owned()), "Begründung: Umlaut-Rückführung „Bäum“ → „Baum“: {found:?}");
    assert_eq!(
        lemma_candidates("Hirten"),
        ["Hirten", "Hirte", "Hirt"],
        "Begründung: ohne Umlaut bleibt die Probe wie seit Runde 17b (längere Stämme zuerst)"
    );
}

#[test]
fn compound_tails_go_from_long_to_short_and_keep_a_head() {
    let tails = compound_tails("Feigenbaum");
    assert_eq!(tails.first().map(String::as_str), Some("Genbaum"), "Begründung: vorderer Teil mindestens 3 Zeichen");
    assert_eq!(tails.last().map(String::as_str), Some("Baum"), "Begründung: kürzester Bestandteil 4 Zeichen, zuletzt");
    assert!(compound_tails("Baum").is_empty(), "Begründung: ein kurzes Wort hat keinen Bestandteil zum Probieren");
}

#[test]
fn strong_numbers_are_recognised_by_their_shape() {
    for key in ["G26", "g26", "H0430", "H430a"] {
        assert!(strong_like(key), "Begründung: Buchstabe G/H und Ziffern sind eine Strong-Nummer ({key})");
    }
    for key in ["Gott", "G", "26", "Hirte"] {
        assert!(!strong_like(key), "Begründung: kein Präfix mit Ziffern, also keine Strong-Nummer ({key})");
    }
}

// ---- Lexikon-Lookup mit Probe --------------------------------------------------------------------------

#[test]
fn the_lemma_probe_finds_baum_for_baeume_and_falls_back_to_the_last_part_of_a_compound() {
    let (_dir, root) = root();
    let lex = lexicon(
        &root,
        &LexiconFixture::new("LexProbe", LexLayout::RawLD4)
            .conf("Description", "Erfundenes Probelexikon")
            .article("Baum", "Ein Gewächs mit Stamm und Krone (erfunden).")
            .article("Ufer", "Rand des Wassers (erfunden)."),
    );
    assert!(lex.lookup("Bäume").unwrap().is_none(), "Begründung: ohne Probe gibt es „Bäume“ nicht als Stichwort");
    let (article, probe) = lex.lookup_lemma("Bäume").unwrap().expect("Bäume findet Baum");
    assert_eq!(article.key, "BAUM", "Begründung: Umlaut-Rückführung trifft das Stichwort „Baum“: {article:?}");
    assert_eq!(probe.as_deref(), Some("Baum"), "Begründung: die Antwort nennt die probierte Form");

    let (article, probe) = lex.lookup_lemma("Feigenbäume").unwrap().expect("Kompositum fällt auf Baum zurück");
    assert_eq!(article.key, "BAUM", "Begründung: letzter Bestandteil „-bäume“ → „Baum“ als Rückfall: {article:?}");
    assert_eq!(probe.as_deref(), Some("Baum"), "Begründung: probierte Form genannt");

    let (article, probe) = lex.lookup_lemma("Ufer").unwrap().expect("Stichwort selbst");
    assert_eq!((article.key.as_str(), probe), ("UFER", None), "Begründung: das Stichwort selbst braucht keine Probe");
    assert!(lex.lookup_lemma("Morgenrot").unwrap().is_none(), "Begründung: nichts passt, ehrlich kein Artikel");
}

#[test]
fn g26_finds_g0026_with_and_without_strongs_padding() {
    let (_dir, root) = root();
    let unpadded = lexicon(
        &root,
        &LexiconFixture::new("LexOhnePolster", LexLayout::RawLD)
            .conf("StrongsPadding", "false")
            .article("G0026", "Liebe (erfundener Artikel).")
            .article("G3588", "Artikel (erfunden)."),
    );
    assert!(
        unpadded.lookup("G26").unwrap().is_none(),
        "Begründung: bei StrongsPadding=false polstert sword-modules „G26“ nicht, das Stichwort „G0026“ fehlt ohne Probe"
    );
    let (article, probe) = unpadded.lookup_lemma("G26").unwrap().expect("G26 findet G0026");
    assert_eq!(article.key, "G0026", "Begründung: die Probe versucht die gepolsterte Schreibweise: {article:?}");
    assert_eq!(probe.as_deref(), Some("G0026"), "Begründung: die gefundene Schreibweise wird genannt");
    assert!(article.text.contains("Liebe"), "Begründung: Text des Artikels: {article:?}");

    let padded = lexicon(
        &root,
        &LexiconFixture::new("LexPolster", LexLayout::ZLD).article("G0026", "Liebe (erfunden, gepolstert)."),
    );
    let (article, probe) = padded.lookup_lemma("G26").unwrap().expect("mit Polster direkt");
    assert_eq!(
        (article.key.as_str(), probe),
        ("G0026", None),
        "Begründung: mit StrongsPadding (Vorgabe) polstert schon die Suche selbst"
    );
    assert!(padded.lookup_lemma("G27").unwrap().is_none(), "Begründung: eine fehlende Nummer bleibt fehlend");
}

// ---- Module mit Trennresten ----------------------------------------------------------------------------

#[test]
fn hyphen_remnants_split_long_words_in_the_middle() {
    assert_eq!(
        hyphen_remnants("Dieses Wort gilt allen Menschen.", 1),
        "Die ses Wort gilt allen Mens chen.",
        "Begründung: Wörter ab sechs Buchstaben werden in der Mitte getrennt, das Satzzeichen bleibt am Wort"
    );
    assert_eq!(
        hyphen_remnants("Dieses Wort gilt allen Menschen.", 2),
        "Dieses Wort gilt allen Mens chen.",
        "Begründung: nur jedes zweite lange Wort"
    );
}

/// Zehn erfundene Verse; jeder zweite mit Trennresten, die übrigen sauber (dort stehen die ganzen Wörter).
fn verses(remnants: bool) -> Vec<(u16, String)> {
    let base = [
        "Am Morgen gingen die Fischer hinunter zum Ufer und warteten.",
        "Dieses Wort sagten die Fischer allen Menschen am Hafen.",
        "Die Menschen hörten dieses Wort und gingen heimwärts.",
        "Am Abend leuchteten Laternen über dem stillen Wasser.",
        "Die Fischer warteten geduldig auf günstigen Wind.",
        "Menschen standen am Ufer und betrachteten die Laternen.",
        "Dieses Licht leuchtete hinaus über den ganzen Hafen.",
        "Geduldig flickten die Fischer ihre Netze am Morgen.",
        "Günstigen Wind erhofften die Menschen jeden Abend.",
        "Die Laternen verloschen erst spät nach Mitternacht.",
    ];
    base.iter()
        .enumerate()
        .map(|(i, t)| {
            let text = if remnants && i % 2 == 1 { hyphen_remnants(t, 1) } else { (*t).to_owned() };
            (u16::try_from(i + 1).unwrap(), text)
        })
        .collect()
}

#[test]
fn a_sample_of_a_module_with_remnants_is_fragmented_and_a_clean_one_is_not() {
    for (name, remnants, fragmented) in [("Trennrest", true, true), ("Sauber", false, false)] {
        let (_dir, root) = root();
        let mut fixture = Fixture::new(name, Layout::ZText(Block::Book), german()).conf("Description", "Erfunden");
        for (verse, text) in verses(remnants) {
            fixture = fixture.verse("Ps", 119, verse, &text);
        }
        write(&root, &fixture).unwrap();
        let found = scan(std::slice::from_ref(&root));
        let bible = open(found.find(name).unwrap()).unwrap();
        let (sample, truncated) = bible.sample_texts(1000, Instant::now() + Duration::from_secs(5)).unwrap();
        assert_eq!(sample.len(), 10, "Begründung: alle zehn Verse in der Stichprobe ({name})");
        assert!(!truncated, "Begründung: unter der Grenze nicht abgebrochen ({name})");
        let survey = fragment_survey(sample.iter().map(String::as_str));
        assert_eq!(
            survey.fragmented(),
            fragmented,
            "Begründung: Trennreste neben ganzen Wörtern machen das Modul auffällig, ein sauberes nicht ({name}): \
             {survey:?}"
        );
        let (cut, truncated) = bible.sample_texts(3, Instant::now() + Duration::from_secs(5)).unwrap();
        assert!(cut.len() == 3 && truncated, "Begründung: die Grenze der Stichprobe gilt ({name})");
    }
}
