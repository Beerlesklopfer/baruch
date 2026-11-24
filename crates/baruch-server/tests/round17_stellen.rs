// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 17b („stellen“): Stellen im Fließtext, Zitat-Zuordnung, Lexikon-Stichwörter und Predigt-Reste –
//! über die Wege, die die Oberfläche und die Personas nehmen:
//! - `bible_research::research` meldet „Psalm 139, … Vers 23“ als Ps 139,23 (nicht das ganze Kapitel) und findet
//!   Lexikon-Artikel über die Lemma-Probe bzw. über die Strong-Nummern der Verse einer Bibel mit Auszeichnung.
//! - `scripture.check` liest Verslisten mit Komma, keine Uhrzeiten und keine Anzahlen.
//! - Predigttext: genau 400 Verse sind nicht gekürzt, 401 schon; Typst-Sonderzeichen im Verstext überstehen den
//!   Weg Predigttext → `#quote` → Wortzählung und Wortlautprüfung.
//! - Zeitwächter (L6): Das Widget schickt den Quelltext (Ereignis `baruch:serialize-text`), der Server zählt ihn
//!   wie beim Laden der Seite; Stichwortzettel (L7): Abkürzungen und `@`-Verweise; Antrag „neuer Teil“ in einer
//!   Predigt (L8): „Predigtteile“ und Nummer in Schemaform.
//!
//! Alle Texte, Bibeln und Lexika sind frei erfunden und werden im Temp-Ordner geschrieben
//! (`baruch_sword::fixture`); kein Modell, kein Netz.

use crate::common;

use std::path::Path;

use baruch_scripture::versification::german;
use baruch_server::bible_research::{NoteKind, research};
use baruch_store::{AssistantAdd, IdeaOrigin, NewPoint, Place};
use baruch_sword::fixture::{Block, Fixture, Layout, LexLayout, LexiconFixture, write, write_lexicon};
use baruch_typst::timing::spoken_words;
use serde_json::{Value, json};

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

/// Modulordner mit Bibeln und Lexika im Temp-Ordner setzen.
async fn modules(server: &common::Server, dir: &Path, bibles: &[Fixture], lexicons: &[LexiconFixture]) {
    let root = dir.join("sword");
    std::fs::create_dir_all(&root).unwrap();
    for m in bibles {
        write(&root, m).unwrap();
    }
    for m in lexicons {
        write_lexicon(&root, m).unwrap();
    }
    ok(server, "bible.set_dirs", json!({ "dirs": [root.display().to_string()] })).await;
}

/// Erfundene Bibel in deutscher Zählung: von Psalm 139 nur Vers 23 und 24.
fn heath_bible() -> Fixture {
    Fixture::new("Heidebibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Heidebibel")
        .conf("DistributionLicense", "Public Domain")
        .verse("Ps", 139, 23, "Prüfe den Pfad durch die Heide und kenne die Hirten.")
        .verse("Ps", 139, 24, "Führe die Schafe zum Brunnen am Abend.")
}

fn heath_lexicon() -> LexiconFixture {
    LexiconFixture::new("Heidelexikon", LexLayout::ZLD)
        .conf("Description", "Erfundenes Heidelexikon")
        .conf("DistributionLicense", "Public Domain")
        .article("Hirte", "Wer Schafe hütet und führt.")
        .article("Schaf", "Herdentier der Heide.")
        .article("Brunnen", "Gegrabene Wasserstelle.")
}

// ---- Stellen im Fließtext und Belege -------------------------------------------------------------------

#[tokio::test]
async fn research_reports_psalm_139_verse_23_instead_of_the_whole_chapter() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[heath_bible()], &[]).await;
    let text = "= Gebet\n\nIm Psalm 139, besonders Vers 23, bittet David um Prüfung.\n";
    let work = server.import(text, "gebet.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Heidebibel" })).await;
    let found = research(&server.state, work, "@belege", text, 10_000).await.expect("@belege bekommt Belege");
    let bible: Vec<(&str, &str)> = found
        .notes
        .iter()
        .filter(|n| n.kind == NoteKind::Bible)
        .map(|n| (n.reference.as_str(), n.text.as_str()))
        .collect();
    assert_eq!(
        bible,
        [("Ps 139,23", "Prüfe den Pfad durch die Heide und kenne die Hirten.")],
        "Kapitel und Vers im selben Satz sind eine Stelle (Backlog R17: vorher „Ps 139“ mit allen Versen)"
    );
    assert!(
        !found.problems.iter().any(|p| p.contains("fehlen")),
        "kein Hinweis auf fehlende Verse – gelesen wird nur Vers 23: {:?}",
        found.problems
    );
}

#[tokio::test]
async fn scripture_check_reads_comma_lists_and_skips_clock_times_and_counts() {
    let server = common::open().await;
    let text = "Treffen Mi 19:30 im Saal; Johannes 3 Mal gesagt.\nLies Joh 3,16, 18 und Psalm 139, V. 23.\n";
    let checked = ok(&server, "scripture.check", json!({ "text": text })).await;
    let refs: Vec<&str> =
        checked["refs"].as_array().unwrap().iter().map(|r| r["normalized"].as_str().unwrap_or("")).collect();
    assert_eq!(
        refs,
        ["Joh 3,16", "Joh 3,18", "Ps 139,23"],
        "Uhrzeit und Anzahl sind keine Stellen, die Komma-Liste zwei, „Psalm 139, V. 23“ eine: {checked}"
    );
}

#[tokio::test]
async fn the_lemma_probe_finds_articles_for_inflected_words() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[], &[heath_lexicon()]).await;
    let text = "= Weide\n\nDie Hirten tränken die Schafe am Brunnen. Bäume stehen still.\n";
    let work = server.import(text, "weide.typ").await.work;
    ok(&server, "bible.set_work_sources", json!({ "work": work, "lexicons": ["Heidelexikon"] })).await;
    let found = research(&server.state, work, "@belege", text, 10_000).await.expect("Lexikon gewählt: Belege");
    let keys: Vec<&str> =
        found.notes.iter().filter(|n| n.kind == NoteKind::Lexicon).map(|n| n.reference.as_str()).collect();
    assert_eq!(
        keys,
        ["HIRTE", "SCHAF", "BRUNNEN"],
        "„Hirten“ → HIRTE, „Schafe“ → SCHAF (Endung abgeschnitten), „Brunnen“ wörtlich; „Bäume“ findet nichts \
         (seit Runde 19c probiert die Probe „Baum“, das Lexikon hat aber kein solches Stichwort)"
    );
}

#[tokio::test]
async fn strong_numbers_of_the_verses_lead_to_lexicon_articles() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let bible = Fixture::new("Starkbibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Bibel mit Strong-Nummern")
        .conf("DistributionLicense", "Public Domain")
        .conf("GlobalOptionFilter", "OSISStrongs")
        .verse("John", 3, 16, "<w lemma=\"strong:G0026\">Die Güte</w> <w lemma=\"strong:G3588\">trägt</w> den Kiesel.");
    let prefixed = LexiconFixture::new("Starklexikon", LexLayout::ZLD)
        .conf("Description", "Erfundenes Lexikon mit Präfix")
        .conf("DistributionLicense", "Public Domain")
        .article("G0026", "Erfundener Artikel zur Nummer G26.");
    let bare = LexiconFixture::new("Zahllexikon", LexLayout::ZLD)
        .conf("Description", "Erfundenes griechisches Lexikon mit bloßen Zahlen")
        .conf("DistributionLicense", "Public Domain")
        .conf("Feature", "GreekDef")
        .article("03588", "Erfundener Artikel zur Zahl 3588.");
    let hebrew = LexiconFixture::new("Hebraeischlexikon", LexLayout::ZLD)
        .conf("Description", "Erfundenes hebräisches Lexikon mit bloßen Zahlen")
        .conf("DistributionLicense", "Public Domain")
        .conf("Feature", "HebrewDef")
        .article("00026", "Hebräischer Artikel, darf für G26 nicht kommen.");
    modules(&server, dir.path(), &[bible], &[prefixed, bare, hebrew]).await;
    let text = "= Güte\n\nSo steht es in Joh 3,16.\n";
    let work = server.import(text, "guete.typ").await.work;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Starkbibel" })).await;
    ok(
        &server,
        "bible.set_work_sources",
        json!({ "work": work, "lexicons": ["Starklexikon", "Zahllexikon", "Hebraeischlexikon"] }),
    )
    .await;
    let found = research(&server.state, work, "@belege", text, 10_000).await.expect("Belege");
    let lexicon: Vec<(&str, &str)> = found
        .notes
        .iter()
        .filter(|n| n.kind == NoteKind::Lexicon)
        .map(|n| (n.module.as_str(), n.reference.as_str()))
        .collect();
    assert_eq!(
        lexicon,
        [("Starklexikon", "G0026"), ("Zahllexikon", "03588")],
        "die Strong-Nummern kommen aus der Auszeichnung des Verses (Bible::verse_strongs), nicht aus dem Text; die \
         bloße Zahl nur im griechischen Lexikon (Feature=GreekDef), nie im hebräischen: {:?}",
        found.notes
    );
}

#[tokio::test]
async fn scripture_check_assigns_nested_quotes_and_block_quotes_in_scenes() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let bible = Fixture::new("Bachbibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Bachbibel")
        .conf("DistributionLicense", "Public Domain")
        .verse("John", 3, 16, "Der Kiesel sprach: Komm, lag am Bach.");
    modules(&server, dir.path(), &[bible], &[]).await;
    for text in [
        "Er las: „Der Kiesel sprach: ‚Komm‘, lag am Bach.“ (Joh 3,16) Und weiter.",
        "#szene(ort: [Bach])[\n  #quote(block: true, attribution: [Joh 3,16])[Der Kiesel sprach: Komm, lag am Bach.]\n]\n",
        "Er las: „Der Kiesel sprach:\n\n„Komm, lag am Bach.“ (Joh 3,16)",
    ] {
        let checked = ok(&server, "scripture.check", json!({ "text": text, "module": "Bachbibel" })).await;
        assert_eq!(
            checked["refs"][0]["wording_code"], "match",
            "„{text}“: das Zitat (verschachtelt, in der Szene bzw. über zwei Absätze) wird der Stelle zugeordnet: {checked}"
        );
    }
}

// ---- Stichwortzettel (L7) ------------------------------------------------------------------------------

#[tokio::test]
async fn notes_keep_abbreviations_in_the_first_sentence_and_drop_references_after_brackets() {
    let server = common::open().await;
    let text = "= Weg\n\nWir gehen z. B. mit Brot (@abb) los, vgl. Kap. 3 und mehr. Der Rest fällt weg.\n\n\
                Schreib an a@b.example, d. h. sofort. Danach mehr.\n";
    let work = server.import(text, "weg.typ").await.work;
    let notes = ok(&server, "notes.get", json!({ "work": work })).await;
    let card = &notes["cards"][0];
    assert_eq!(card["rule"], "first_sentence", "weder Markierung noch Hervorhebung: {card}");
    let points: Vec<&str> = card["points"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert_eq!(
        points,
        ["Wir gehen z. B. mit Brot () los, vgl. Kap. 3 und mehr.", "Schreib an a@b.example, d. h. sofort."],
        "„z. B.“, „vgl.“, „Kap.“, „d. h.“ beenden den Satz nicht; „(@abb)“ ist ein Verweis, „a@b.example“ bleibt"
    );
}

// ---- Zeitwächter (L6) ----------------------------------------------------------------------------------

#[tokio::test]
async fn the_live_count_of_the_source_text_equals_the_count_on_page_load() {
    let server = common::open().await;
    // Typst-Inseln mit und ohne gesprochenen Text: Fußnote und Arbeitsauftrag zählen nicht, die Szene schon.
    let text = "= Inseln\n\nDer Hirte ruft.#footnote[Anmerkung des Autors]\n\n#szene(ort: [Heide])[\n  Die Schafe \
                hören.\n]\n\n#arbeit[noch prüfen] Dann gehen alle heim.\n";
    let work = server.import(text, "inseln.typ").await.work;
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    let on_load = timing["sections"][0]["words"].as_u64().unwrap();
    let loaded = server.state.store.load_work(work).await.unwrap();
    let source = loaded.nodes[0].revisions[loaded.nodes[0].current_index().unwrap()].body.clone();
    let live = ok(&server, "timing.count", json!({ "work": work, "text": source })).await;
    assert_eq!(
        live["words"].as_u64(),
        Some(on_load),
        "der Quelltext (über baruch:serialize-text) zählt im Editorkopf wie beim Laden der Seite: {live}"
    );
    assert_eq!(on_load, spoken_words(&source) as u64, "dieselbe Zählregel baruch_typst::timing::spoken_words");
    assert_eq!(on_load, 10, "„Der Hirte ruft. Die Schafe hören. Dann gehen alle heim.“ = 10 Wörter");
    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "timing.js").expect("Widget timing.js");
    assert!(
        script.1.contains("baruch:serialize-text"),
        "das Widget fragt das Satzbild nach dem Quelltext (Vertrag §2.6), statt innerText zu zählen"
    );
    assert!(script.1.contains("timing.count"), "gezählt wird weiter nur auf dem Server");
}

// ---- Antrag „neuer Teil“ in einer Predigt (L8) ---------------------------------------------------------

#[tokio::test]
async fn a_new_part_in_a_sermon_moves_sermon_parts_and_shows_the_schema_number() {
    let server = common::open().await;
    let created = ok(&server, "work.create", json!({ "title": "Heidepredigt", "kind": "sermon" })).await;
    let work = created["work"].as_i64().unwrap();
    let loaded = server.state.store.load_work(work).await.unwrap();
    let target = loaded.nodes.iter().find(|n| n.number == "I.").expect("Predigtteil I.").id;
    // Alle Punkte ab „I.“ sind Predigtteile (Ebene 1) und wandern unter den neuen Teil.
    let adopted_count = loaded.nodes.len() - target;
    let added = server
        .state
        .store
        .outline_add_by_assistant(
            work,
            Some(target),
            Place::Before,
            &[NewPoint { title: "Hauptteil".into(), level: Some(0) }],
            &IdeaOrigin::Mcp { handle: "@mcp:pruefer".into() },
            "nachbau",
            "",
        )
        .await
        .unwrap();
    let AssistantAdd::Requested(request) = added else {
        panic!("ein Teil vor Predigtteil I. übernimmt die folgenden Predigtteile und wird ein Antrag");
    };
    assert!(
        request.summary.contains(&format!("{adopted_count} vorhandene Predigtteile wandern")),
        "in einer Predigt heißen die Punkte der Ebene 1 Predigtteile, nicht Kapitel: {}",
        request.summary
    );
    assert!(!request.summary.contains("Kapitel"), "kein „Kapitel“ in der Predigt: {}", request.summary);
    let listed = ok(&server, "outline.requests", json!({ "work": work, "preview": true })).await;
    let preview = listed["requests"][0]["preview"].as_array().unwrap().clone();
    let first = preview.iter().find(|p| p["title"] == "Beobachtung am Text").expect("Predigtteil in der Vorschau");
    let number = first["number"].as_str().unwrap();
    assert!(number.ends_with('.'), "das Schema der Predigt schreibt die Nummer mit Punkt: {first}");
    assert_eq!(
        first["label"].as_str().unwrap(),
        format!("Predigtteil {number}"),
        "die Vorschau zeigt die Nummer in der Form des Schemas (Backlog: vorher „Predigtteil I“ ohne Punkt): {first}"
    );
    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(page.contains("vorhandene Predigtteile wandern"), "auch die Seite ohne JavaScript nennt Predigtteile");
}

// ---- Predigttext: 400-Verse-Grenze und Typst-Sonderzeichen ---------------------------------------------

/// Die ersten `n` Verse von 1. Mose in deutscher Zählung (Kapitel, Vers).
fn genesis_verses(n: usize) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    let mut chapter = 1;
    while out.len() < n {
        let last = german().verses_in("Gen", chapter).expect("1. Mose hat genug Kapitel");
        for verse in 1..=last {
            if out.len() < n {
                out.push((chapter, verse));
            }
        }
        chapter += 1;
    }
    out
}

/// Erfundene Bibel mit den ersten 401 Versen von 1. Mose (je Vers ein eigener Satz).
fn long_bible() -> Fixture {
    genesis_verses(401).into_iter().fold(
        Fixture::new("Langbibel", Layout::ZText(Block::Book), german())
            .conf("Description", "Erfundene Langbibel")
            .conf("DistributionLicense", "Public Domain"),
        |bible, (c, v)| bible.verse("Gen", c, v, &format!("Stein {c} liegt bei Stein {v}.")),
    )
}

#[tokio::test]
async fn a_sermon_text_of_exactly_400_verses_is_complete_and_401_verses_are_cut() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    modules(&server, dir.path(), &[long_bible()], &[]).await;
    for (count, cut) in [(400usize, false), (401, true)] {
        let (c, v) = *genesis_verses(count).last().unwrap();
        let text_ref = format!("1. Mo 1,1–{c},{v}");
        let created = ok(
            &server,
            "work.create",
            json!({ "title": format!("Langpredigt {count}"), "kind": "sermon", "text_ref": text_ref, "module": "Langbibel" }),
        )
        .await;
        let text = &created["sermon_text"];
        assert_eq!(text["quoted"], true, "{count} Verse: der Wortlaut steht da: {text}");
        assert_eq!(text["verses"].as_u64(), Some(400), "{count} Verse: eingefügt werden höchstens 400: {text}");
        let notice = text["notice"].as_str().unwrap_or("");
        assert_eq!(
            notice.contains("Nur die ersten 400 Verse"),
            cut,
            "{count} Verse: „gekürzt“ nur, wenn die Stelle mehr als 400 Verse hat (read_passage.truncated; vorher galten \
             genau 400 als gekürzt): {text}"
        );
    }
}

#[tokio::test]
async fn typst_special_characters_in_the_verse_survive_quote_word_count_and_wording_check() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let verse = "Lobt #1 den *Hirten* _hier_ für $5 an @tor <weg> [x] mit \\ Stab~Ende.";
    // Im Modul steht „<“ als Entität (ein rohes „<weg>“ wäre Auszeichnung und fiele beim Lesen weg).
    let stored = verse.replace('<', "&lt;").replace('>', "&gt;");
    let bible = Fixture::new("Zeichenbibel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene Zeichenbibel")
        .conf("DistributionLicense", "Public Domain")
        .verse("John", 10, 11, &stored);
    modules(&server, dir.path(), &[bible], &[]).await;
    let created = ok(
        &server,
        "work.create",
        json!({ "title": "Zeichenpredigt", "kind": "sermon", "text_ref": "Joh 10,11", "module": "Zeichenbibel" }),
    )
    .await;
    let work = created["work"].as_i64().unwrap();
    let loaded = server.state.store.load_work(work).await.unwrap();
    let first = &loaded.nodes[0];
    let body = &first.revisions[first.current_index().unwrap()].body;
    assert!(
        body.contains(
            "[Lobt \\#1 den \\*Hirten\\* \\_hier\\_ für \\$5 an \\@tor \\<weg\\> \\[x\\] mit \\\\ Stab\\~Ende.]"
        ),
        "jedes Typst-Zeichen im Verstext ist im #quote maskiert (sonst würde es Code, Mathe, Verweis oder Label): {body}"
    );
    // Wörter des Verses: Folgen ohne Leerraum mit Buchstabe oder Ziffer („\“ allein ist keins); „Stab~Ende.“
    // ist im Bibeltext ein Wort, im Typst-Quelltext aber geschütztes Leerzeichen – maskiert bleibt es eins.
    let expected = verse.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count();
    let timing = ok(&server, "work.timing", json!({ "work": work })).await;
    assert_eq!(
        timing["sections"][0]["words"].as_u64(),
        Some(expected as u64),
        "der Zeitwächter zählt den maskierten Predigttext wie den Verstext (Zuschreibung und Überschrift nicht): {timing}"
    );
    let check = ok(&server, "bible.check", json!({ "work": work, "key": first.key })).await;
    assert_eq!(
        check["refs"][0]["wording_code"], "match",
        "die Wortlautprüfung entfernt die Maskierung wieder und bestätigt das eingefügte Zitat: {check}"
    );
}
