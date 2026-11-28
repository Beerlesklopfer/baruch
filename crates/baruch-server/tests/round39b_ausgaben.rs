// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b „User interface German and English, outputs and help“, owner ausgaben: book output in the work language
//! (EPUB, Typst/PDF, DOCX, notes sheet, read-back words), golden German fixtures and the book catalog completeness
//! check (contract §2.1, §4.1, §4.2).
//!
//! Golden fixtures (`tests/data/round39b/`): generated with the unchanged S1 code (commit 5c5a821) from a fixed German
//! work before any work-language change. Every later change must keep them byte-equal, because German works must not
//! change. Regenerating them is forbidden after the first commit of the round; `BARUCH_BLESS_R39B` only exists for
//! that first commit. Texts freely invented.

use std::path::PathBuf;

use crate::common;

/// Compares `actual` with the fixture `name` (or writes it while blessing the first commit).
fn golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/round39b").join(name);
    if std::env::var_os("BARUCH_BLESS_R39B").is_some() {
        std::fs::create_dir_all(path.parent().expect("fixture folder")).expect("fixture folder can be created");
        std::fs::write(&path, actual).expect("fixture can be written");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("golden fixture {} is missing ({e}); German works must not change", path.display()));
    if expected != actual {
        let line = expected.lines().zip(actual.lines()).position(|(a, b)| a != b).map_or(0, |i| i + 1);
        panic!(
            "German works must not change: {name} differs from the golden fixture (first differing line {line}).\n\
             --- expected\n{expected}\n--- actual\n{actual}"
        );
    }
}

/// The fixed test work: a part, three chapters (one with a section), a footnote, a cross-reference and a Bible
/// reference.
pub fn source(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Erika Beispiel\")\n\
         #set text(lang: \"de\")\n\n\
         {part}\n\
         = Die Weide\n\nAm Bach steht eine alte Weide.#footnote[Erfunden.] Wie Ps 46,4 sagt.\n\n\
         == Boden und Licht\n\nDer Boden ist lehmig.\n\n\
         = Das Fundament\n\nSechs Punktfundamente tragen das Haus.\n\n\
         = Das Dach\n\nZum Schluss kommt das Dach.\n",
        part = baruch_typst::part_line("Planen"),
    )
}

/// Every output that does not depend on the clock or the random work id, for every scheme with words.
async fn outputs(server: &common::Server, work: i64) -> String {
    let store = &server.state.store;
    let mut out = String::new();
    for scheme in ["parts_chapters", "chapter_words", "legal", "sermon", "book"] {
        store.set_numbering_scheme(work, scheme, None).await.expect("the scheme is valid");
        out.push_str(&format!("==== scheme {scheme}\n"));
        out.push_str(&store.export_typst(work).await.expect("the work exports"));
        out.push_str("\n==== notes\n");
        let notes = baruch_server::pdf::notes::notes_of(&server.state, work).await.expect("notes of the work");
        out.push_str(&baruch_render::notes::notes_source(&notes.doc(), baruch_render::notes::NotesFormat::A5, 16));
    }
    let mut input = baruch_server::epub::epub_input(&server.state, work, baruch_epub::FootnoteMode::Endnotes)
        .await
        .expect("the EPUB input of the work");
    input.meta.identifier = "urn:uuid:00000000-0000-4000-8000-000000000039".into();
    input.meta.modified = "2026-11-28T12:00:00Z".into();
    let book = baruch_epub::build(&input).expect("the EPUB builds");
    for item in &book.items {
        out.push_str(&format!("==== {} ({:?})\n", item.href, item.kind));
        out.push_str(&String::from_utf8_lossy(&item.bytes));
        out.push('\n');
    }
    out.push_str(&format!("==== opf\n{}\n", book.opf));
    out
}

#[tokio::test]
async fn a_german_work_keeps_its_export_notes_and_epub() {
    let server = common::open().await;
    let store = &server.state.store;
    let work = store.import_typst(&source("Das Gartenhaus"), "gartenhaus.typ", "Erika").await.expect("import").work;
    let settings = baruch_typst::typeset::TypesetSettings {
        index_pages: Some(baruch_typst::typeset::IndexPages { registers: vec!["personen".into()], columns: 2 }),
        ..Default::default()
    };
    store.set_work_typeset(work, &settings, "Erika").await.expect("typeset settings are valid");
    golden("server_work.de.txt", &outputs(&server, work).await);
}

// ---- Book catalog (contract §4.2) ----------------------------------------------------------------------------------

/// Keys `"book.…"` used as string literals in `crates/*/src/**/*.rs` and `crates/baruch-server/web/src/*.ts`, with the
/// file of the first use. `baruch-core/src/book_text.rs` is left out: its unit tests name every key and would make
/// every key look used.
fn used_book_keys() -> std::collections::BTreeMap<String, String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "ts") {
                out.push(path);
            }
        }
    }
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("crates folder") {
        let src = entry.expect("crate").path().join("src");
        if src.is_dir() {
            walk(&src, &mut files);
        }
    }
    walk(&crates.join("baruch-server/web/src"), &mut files);
    let mut used = std::collections::BTreeMap::new();
    for file in files.iter().filter(|f| !f.ends_with("baruch-core/src/book_text.rs")) {
        let text = std::fs::read_to_string(file).expect("source file is UTF-8");
        let mut rest = text.as_str();
        while let Some(at) = rest.find("\"book.") {
            let tail = &rest[at + 1..];
            let end = tail.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'));
            let key = &tail[..end.unwrap_or(tail.len())];
            // Only keys of the catalog form (two segments after `book`, or the seed `book.untitled`) closed by a quote.
            let closed = end.is_some_and(|e| tail[e..].starts_with('"'));
            if closed && (key.split('.').count() >= 3 || key == "book.untitled") {
                used.entry(key.to_owned()).or_insert_with(|| file.display().to_string());
            }
            rest = &tail[key.len()..];
        }
    }
    used
}

#[test]
fn every_book_key_used_exists_and_every_catalog_key_is_used() {
    use baruch_core::book_text::{book_catalog_problems, book_exists, book_keys};
    assert_eq!(book_catalog_problems(), Vec::<String>::new(), "both book catalogs parse cleanly with the same keys");
    let used = used_book_keys();
    for (key, file) in &used {
        assert!(book_exists(key), "`{key}` (used in {file}) is missing in the book catalog; book_t would show the key");
    }
    for key in book_keys() {
        assert!(used.contains_key(key), "`{key}` is in the book catalog but no source uses it (dead entry)");
    }
}

#[test]
fn no_seed_key_was_renamed_and_no_german_seed_value_changed() {
    use baruch_core::WorkLanguage::De;
    use baruch_core::book_text::book_t;
    // The constants the code wrote before Round 39b: German works must not change, so every German catalog value
    // that replaced one must be byte-equal to it.
    let pairs = [
        ("book.imprint.dnb_title", baruch_typst::cover::DNB_TITLE.to_owned()),
        ("book.imprint.dnb_text", baruch_typst::cover::DNB_TEXT.to_owned()),
        ("book.imprint.typeset_with", baruch_typst::cover::TYPESET_WITH.to_owned()),
        ("book.draft.watermark", baruch_typst::typeset::DEFAULT_DRAFT_TEXT.to_owned()),
        ("book.figure.supplement", baruch_epub::xhtml::FIGURE_SUPPLEMENT.to_owned()),
        ("book.scripture.unchecked", baruch_server::scripture::UNCHECKED.to_owned()),
        ("book.scripture.mark_scripture", baruch_server::scripture::MARK_SCRIPTURE.to_owned()),
        ("book.scripture.mark_others", baruch_server::scripture::MARK_OTHERS.to_owned()),
        ("book.scripture.mark_bible", baruch_server::scripture::MARK_BIBLE.to_owned()),
        ("book.claims.in_text", baruch_server::assist::ref_claims::IN_TEXT.to_owned()),
        ("book.claims.invalid", baruch_server::assist::ref_claims::INVALID.to_owned()),
        ("book.websearch.not_supported", baruch_server::websearch::verify::NOT_BELEGT.to_owned()),
        ("book.websearch.unknown_source", baruch_server::websearch::verify::UNKNOWN_SOURCE.to_owned()),
        ("book.websearch.not_fetched", baruch_server::websearch::verify::NOT_FETCHED.to_owned()),
        ("book.websearch.check_note", baruch_server::websearch::verify::CHECK_NOTE.to_owned()),
    ];
    for (key, literal) in pairs {
        assert_eq!(book_t(De, key, &[]), literal, "German works must not change: `{key}` equals the old literal");
    }
    assert_eq!(
        baruch_server::scripture::mark_others_bible_in("GerSch", De),
        baruch_server::scripture::mark_others_bible("GerSch"),
        "the closing line with a Bible stays German for German works"
    );
    assert!(
        baruch_server::bibles::SPACING_LABEL
            .ends_with(&format!("({})", book_t(De, "book.scripture.module_spacing", &[]))),
        "the wording label with whitespace in the module ends with the catalog phrase, so English works translate it"
    );
}

// ---- English works (contract §2.1) ---------------------------------------------------------------------------------

/// A work like the German one above with English texts, set to English.
async fn english_work(server: &common::Server) -> i64 {
    let store = &server.state.store;
    let source = format!(
        "#set document(title: \"The Garden House\", author: \"Erin Example\")\n\n\
         {part}\n= The Willow\n\nAn old willow stands by the brook. As Ps 46,4 says.\n\n\
         == Soil and Light\n\nThe soil is loamy.\n\n= The Foundation\n\nSix point foundations.\n\n\
         = The Roof\n\nAt last the roof.\n",
        part = baruch_typst::part_line("Planning"),
    );
    let work = store.import_typst(&source, "garden-house.typ", "Erin").await.expect("import").work;
    store.set_work_language(work, baruch_core::WorkLanguage::En).await.expect("the language is set");
    work
}

#[tokio::test]
async fn an_english_work_exports_english_numbering_words_and_the_english_language_rule() {
    let server = common::open().await;
    let store = &server.state.store;
    let work = english_work(&server).await;
    store.set_numbering_scheme(work, "parts_chapters", None).await.unwrap();
    let export = store.export_typst(work).await.unwrap();
    assert!(
        export.contains("#set text(lang: \"en\")"),
        "an English work without its own typesetting language gets the English language rule: {export}"
    );
    assert!(export.contains("\"Chapter \" + str(p.at(0))"), "the chapter rule writes English words: {export}");
    assert!(!export.contains("\"Kapitel \""), "no German chapter word in an English work: {export}");
    let loaded = store.load_work(work).await.unwrap();
    let numbers: Vec<&str> = loaded.nodes.iter().map(|n| n.number.as_str()).collect();
    assert_eq!(numbers, ["Part I", "Chapter 1", "1.1", "Chapter 2", "Chapter 3"], "the outline numbers are English");

    store.set_numbering_scheme(work, "sermon", None).await.unwrap();
    let notes = baruch_server::pdf::notes::notes_of(&server.state, work).await.unwrap();
    let sheet = baruch_render::notes::notes_source(&notes.doc(), baruch_render::notes::NotesFormat::A5, 16);
    assert!(sheet.contains("\"Cue sheet – The Garden House\"") && sheet.contains("lang: \"en\""), "{sheet}");
    assert!(sheet.contains("\"Introduction\"") && sheet.contains("\"Conclusion\""), "sermon words: {sheet}");

    // The typesetting setting wins over the work language (contract §2.1, rule 1).
    let settings = baruch_typst::typeset::TypesetSettings { lang: Some("de-CH".into()), ..Default::default() };
    store.set_work_typeset(work, &settings, "Erin").await.unwrap();
    let export = store.export_typst(work).await.unwrap();
    assert!(
        export.contains("#set text(lang: \"de\", region: \"CH\")") && !export.contains("#set text(lang: \"en\")"),
        "with its own typesetting language the work gets exactly that rule: {export}"
    );
}

fn docx_part(bytes: &[u8], name: &str) -> String {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("the Word file is a ZIP archive");
    let mut entry = zip.by_name(name).expect("the part is in the archive");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("UTF-8");
    text
}

#[tokio::test]
async fn an_english_work_gets_an_english_epub_and_word_file() {
    let server = common::open().await;
    let work = english_work(&server).await;
    let input =
        baruch_server::epub::epub_input(&server.state, work, baruch_epub::FootnoteMode::Endnotes).await.unwrap();
    assert_eq!(input.work_language, baruch_core::WorkLanguage::En, "the EPUB input carries the work language");
    assert_eq!(input.meta.language, "en", "the EPUB language tag of an English work is en");
    let book = baruch_epub::build(&input).unwrap();
    let nav = book.items.iter().find(|i| i.href == "nav.xhtml").map(|i| String::from_utf8_lossy(&i.bytes).into_owned());
    assert!(nav.is_some_and(|n| n.contains("<h1>Contents</h1>")), "the EPUB navigation is English");

    let options = baruch_server::docx_export::DocxExportOptions::default();
    let file = baruch_server::docx_export::export_file(&server.state, work, &options).await.unwrap();
    let styles = docx_part(&file.bytes, "word/styles.xml");
    assert!(styles.contains("w:lang w:val=\"en-GB\""), "the Word file of an English work is English: {styles}");

    let german = server.state.store.import_typst(&source("Das Gartenhaus"), "gartenhaus.typ", "Erika").await.unwrap();
    let file = baruch_server::docx_export::export_file(&server.state, german.work, &options).await.unwrap();
    assert!(
        docx_part(&file.bytes, "word/styles.xml").contains("w:lang w:val=\"de-DE\""),
        "German works keep the German Word language (regression)"
    );
}

#[tokio::test]
async fn the_editor_of_an_english_work_tells_the_widgets_its_language() {
    let server = common::open().await;
    let work = english_work(&server).await;
    let (status, html) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, axum::http::StatusCode::OK, "the work page opens");
    assert!(
        html.contains("data-work-lang=\"en\""),
        "the editor carries data-work-lang for the Typst snippets of xref.ts and wysiwyg.ts"
    );
    let (_, sample) = server.get("/").await;
    assert!(sample.contains("data-work-lang=\"de\""), "a German work says de (the widgets fall back to de as well)");
}

// ---- Read-back words (contract §2.1) -------------------------------------------------------------------------------

#[test]
fn baruch_writes_the_read_back_words_in_the_work_language_and_reads_both() {
    use baruch_core::WorkLanguage::{De, En};
    use baruch_server::bibles::Choice;
    use baruch_server::scripture::{mark_with, mark_with_in};
    let section = "Wie Joh 3,16 sagt.";
    let answer = "Joh 3,16 passt.".to_owned();
    let english = mark_with_in(&Choice::None, "@lektor", section, answer.clone(), En);
    assert!(
        english.ends_with("Note: Bible references and wording are checked by @schrift; unchecked here."),
        "the closing line of an English work is English: {english}"
    );
    assert_eq!(
        mark_with_in(&Choice::None, "@lektor", section, answer.clone(), De),
        mark_with(&Choice::None, "@lektor", section, answer),
        "German works keep the German closing line (regression)"
    );
    let schrift = mark_with_in(&Choice::None, "@schrift", section, "Joh 3,16 ist korrekt zitiert.".into(), En);
    assert!(schrift.contains("Wording: unchecked"), "@schrift in an English work: {schrift}");
    assert_eq!(
        baruch_server::scripture::wording_label_in("Wortlaut stimmt (Teilzitat, Leerraum im Modul)", En),
        "wording matches (partial quotation, whitespace in the module)",
        "the wording labels of Baruch are translated phrase by phrase"
    );

    let prompt = baruch_server::assist::prompts::default_prompt_in("@schrift", "", baruch_core::WorkKind::Book, En);
    assert!(prompt.contains("\"wording unchecked\"") && !prompt.contains("Wortlaut ungeprüft"), "{prompt}");
    let belege = baruch_server::assist::prompts::default_prompt_in("@belege", "", baruch_core::WorkKind::Book, En);
    assert!(belege.contains("\"not supported\"") && belege.contains("\"check reference\""), "{belege}");
    let german = baruch_server::assist::prompts::default_prompt_in("@belege", "", baruch_core::WorkKind::Book, De);
    assert!(german.contains("\"nicht belegt\"") && german.contains("\"Angabe prüfen\""), "German prompt unchanged");

    let corpus = "Predigttext: Lk 15,4-6\n<<<\nDer Hirte sucht.\n>>>";
    let claim = "Critical: Lk 15,5 is not in the text.";
    let marked = baruch_server::assist::ref_claims::mark_ref_claims(claim, corpus);
    assert!(
        marked.contains("(reference is in the text: Lk 15,4"),
        "an English finding gets the English note: {marked}"
    );
    assert_eq!(
        baruch_server::assist::ref_claims::mark_ref_claims(&marked, corpus),
        marked,
        "the English note is recognised and not written twice"
    );
    let german = baruch_server::assist::ref_claims::mark_ref_claims("Kritisch: Lk 15,5 ist nicht belegt.", corpus);
    assert!(german.contains("(Stelle steht im Text: Lk 15,4"), "German findings keep the German note: {german}");
}

#[test]
fn the_web_evidence_note_follows_the_work_language_and_both_are_recognised() {
    use baruch_core::WorkLanguage::{De, En};
    use baruch_server::websearch::verify::{CHECK_NOTE, check, check_in};
    let research = baruch_server::websearch::Research::default();
    let answer = "The harbour was built in 1890 [W1].";
    let english = check_in(answer, &research, En).text;
    assert!(english.contains("check reference: not supported"), "English note without evidence: {english}");
    assert!(english.contains("[W1] (unknown source)"), "English addition behind an unknown label: {english}");
    assert!(!english.contains(CHECK_NOTE), "no German note in an English work: {english}");
    assert_eq!(check_in(answer, &research, De).text, check(answer, &research).text, "German: as before");
    let again = check_in(&check(answer, &research).text, &research, En).text;
    assert!(!again.contains("(unbekannte Quelle) (unknown source)"), "a German addition is recognised: {again}");
}
