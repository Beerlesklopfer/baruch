// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31b (register-satz): Register und Glossar im EPUB – `text/glossary.xhtml` (glossterm/glossdef),
//! `text/index-{slug}.xhtml` mit Links auf die Anker der Marken, erste Erwähnung als Link ins Glossar. Die Abschnitte
//! kommen wie im Server aus der Projektion (`Projector`, dort über `Store::export_map`). Testwerk frei erfunden.

use baruch_epub::check::{Severity, check_structure, find_xmllint};
use baruch_epub::{EpubBook, EpubInput, ItemKind, build, package, sample};
use baruch_typst::index::{BackMatter, GlossaryEntry, IndexMark, Projector, builtin_registers, mark_source};
use baruch_typst::typeset::{GlossaryPosition, IndexPages, TypesetSettings};

const UUID: &str = "7b0e5c1d-0000-4000-8000-000000000031";

fn back(glossary_page: Option<GlossaryPosition>) -> BackMatter {
    BackMatter {
        settings: TypesetSettings {
            index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "bibel".into()], columns: 2 }),
            glossary_page,
            glossary_first_mention: Some(true),
            ..TypesetSettings::default()
        },
        registers: builtin_registers(),
        glossary: vec![GlossaryEntry {
            uuid: UUID.into(),
            term: "Weide".into(),
            variants: vec!["Salix".into()],
            short: "Baum am Wasser.".into(),
            long: "Sie wächst schnell.\n\nIhre Zweige biegen sich.".into(),
            bible_refs: vec!["Ps 137,2".into()],
        }],
    }
}

/// Testwerk mit Marken in Kapitel 1 und 2 (eine in einer Fußnote) und einer Bibelstelle, projiziert wie im Export.
fn input(glossary_page: Option<GlossaryPosition>) -> EpubInput {
    let mut input = sample::input();
    let lehm = mark_source(&IndexMark::new("stichwort", "Lehm"));
    let bach = mark_source(&IndexMark { sub: "Hochwasser".into(), ..IndexMark::new("stichwort", "Bach") });
    let see = mark_source(&IndexMark { see: "Lehm".into(), ..IndexMark::new("stichwort", "Ton") });
    input.sections[1].body = input.sections[1].body.replacen("Am Bach", &format!("Am Bach{bach}"), 1).replacen(
        "über die Ufer.]",
        &format!("über die Ufer, wie Ps 46,4 sagt.{lehm}]"),
        1,
    );
    input.sections[3].body = input.sections[3].body.replacen("trägt der", &format!("trägt der{see}"), 1).replacen(
        "Lehm schlecht",
        &format!("Lehm{lehm} schlecht"),
        1,
    );
    assert!(input.sections[3].body.contains("Lehm#metadata"), "Marke im Testwerk gesetzt");
    let back = back(glossary_page);
    let mut projector = Projector::new(&back);
    for s in &mut input.sections {
        s.body = projector.project(&s.body).into_owned();
    }
    input.back_matter = back;
    input
}

fn text(book: &EpubBook, href: &str) -> String {
    String::from_utf8(book.item(href).unwrap_or_else(|| panic!("{href} fehlt")).bytes.clone()).unwrap()
}

#[test]
fn glossary_and_register_files_follow_the_naming_rules_and_the_typeset_order() {
    let book = build(&input(Some(GlossaryPosition::AfterIndexes))).expect("baut");
    let glossary = book.item("text/glossary.xhtml").expect("Glossar");
    assert_eq!((glossary.id.as_str(), glossary.kind), ("glossary", ItemKind::Glossary), "Name und Art (Vertrag §2.4)");
    let index = book.item("text/index-stichwort.xhtml").expect("Stichwortregister");
    assert_eq!((index.id.as_str(), index.kind), ("index-stichwort", ItemKind::Index), "Name und Art");
    assert!(book.item("text/index-bibel.xhtml").is_some(), "Bibelstellenregister");
    let tail: Vec<&str> = book.spine.iter().rev().take(3).map(String::as_str).collect();
    assert_eq!(tail, vec!["glossary", "index-bibel", "index-stichwort"], "Spine-Ende: Register, dann Glossar");
    assert!(book.toc.iter().any(|e| e.href == "text/glossary.xhtml" && e.title == "Glossar"), "im Inhalt");
    assert!(book.opf.contains("href=\"text/index-stichwort.xhtml\""), "im Manifest");

    let before = build(&input(Some(GlossaryPosition::BeforeIndexes))).expect("baut");
    let tail: Vec<&str> = before.spine.iter().rev().take(3).map(String::as_str).collect();
    assert_eq!(tail, vec!["index-bibel", "index-stichwort", "glossary"], "Glossar vor den Registern");
    let none = build(&input(None)).expect("baut");
    assert!(none.item("text/glossary.xhtml").is_none(), "ohne Glossarseite kein glossary.xhtml");
}

#[test]
fn the_glossary_uses_glossterm_and_glossdef_and_first_mentions_link_to_it() {
    let book = build(&input(Some(GlossaryPosition::AfterIndexes))).expect("baut");
    let g = text(&book, "text/glossary.xhtml");
    assert!(g.contains("epub:type=\"glossary\""), "epub:type glossary: {g}");
    assert!(
        g.contains(&format!("<dt id=\"baruch-gl-{UUID}\" epub:type=\"glossterm\"><dfn>Weide</dfn></dt>")),
        "Begriff als glossterm mit Label des Satzes: {g}"
    );
    assert!(g.contains("<dd epub:type=\"glossdef\">"), "Erklärung als glossdef");
    for part in ["Auch: Salix", "<p>Baum am Wasser.</p>", "<p>Sie wächst schnell.</p>", "Bibelstellen: Ps 137,2"] {
        assert!(g.contains(part), "{part} im Glossar: {g}");
    }
    let ch1 = text(&book, &format!("text/k{}.xhtml", sample::KEYS[1]));
    assert!(
        ch1.contains(&format!(
            "<a class=\"baruch-glossref\" epub:type=\"glossref\" href=\"glossary.xhtml#baruch-gl-{UUID}\"><em>Weide</em></a>"
        )),
        "erste Erwähnung im Kapitel als Link ins Glossar: {ch1}"
    );
    let without = build(&input(None)).expect("baut");
    let ch1 = text(&without, &format!("text/k{}.xhtml", sample::KEYS[1]));
    assert!(ch1.contains("<em>Weide</em>") && !ch1.contains("glossref"), "ohne Glossarseite nur hervorgehoben");
    assert!(
        !without.warnings.iter().any(|w| w.code == "ISLAND_OMITTED" && w.message.contains("baruch")),
        "keine ausgelassene Insel aus der Projektion: {:?}",
        without.warnings
    );
}

#[test]
fn register_pages_link_to_the_anchors_of_the_marks() {
    let book = build(&input(Some(GlossaryPosition::AfterIndexes))).expect("baut");
    let index = text(&book, "text/index-stichwort.xhtml");
    assert!(index.contains("epub:type=\"index\""), "epub:type index: {index}");
    let [_, ch1, _, ch2, _] = sample::KEYS;
    // Lehm: in einer Fußnote von Kapitel 1 (Popup: dieselbe Datei) und im Text von Kapitel 2.
    let lehm_start = index.find("index-term\">Lehm</span>").expect("Lehm");
    let lehm = &index[lehm_start..index[lehm_start..].find("</li>").unwrap() + lehm_start];
    assert!(
        lehm.contains(&format!("href=\"k{ch1}.xhtml#baruch-idx-{ch1}-"))
            && lehm.contains(&format!("href=\"k{ch2}.xhtml#baruch-idx-{ch2}-")),
        "je Abschnitt ein Link auf den Anker der Marke: {lehm}"
    );
    assert!(lehm.contains(">1 Der Platz</a>") && lehm.contains(">2 Das Fundament</a>"), "Fundstelle = Abschnitt");
    for href in lehm.split("href=\"").skip(1).map(|s| &s[..s.find('"').unwrap()]) {
        let (file, id) = href.split_once('#').expect("Datei#Anker");
        assert!(text(&book, &format!("text/{file}")).contains(&format!("id=\"{id}\"")), "Anker {id} steht in {file}");
    }
    assert!(index.contains("index-term\">Ton</span> <em>siehe</em> Lehm"), "„siehe“ ohne Fundstelle: {index}");
    let bach = index.find("index-term\">Bach</span>").expect("Bach");
    let hoch = index.find("index-term\">Hochwasser</span>").expect("Unterstichwort");
    assert!(bach < hoch, "Unterstichwort unter dem Stichwort");
    assert!(index.contains("<h2 class=\"baruch-index-letter\">B</h2>"), "Buchstabengruppen");
    let bible = text(&book, "text/index-bibel.xhtml");
    let psalms = bible.find("index-term\">Psalmen</span>").expect("Psalmen aus der automatischen Stelle");
    assert!(bible[psalms..].contains("index-term\">46,4</span>"), "Stelle als Unterstichwort: {bible}");
}

#[test]
fn the_book_passes_the_own_check_and_is_byte_identical_for_the_same_input() {
    let book = build(&input(Some(GlossaryPosition::BeforeIndexes))).expect("baut");
    let bytes = package(&book).expect("packt");
    let xmllint = find_xmllint(std::env::var_os("PATH").as_deref());
    let (findings, _) = check_structure(&bytes, xmllint.as_deref());
    let errors: Vec<_> = findings.iter().filter(|f| f.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "keine neuen Fehler der eigenen Prüfung (Links, ids, XHTML): {errors:#?}");
    let again = package(&build(&input(Some(GlossaryPosition::BeforeIndexes))).unwrap()).unwrap();
    assert_eq!(bytes, again, "gleiche Bytes bei gleichem Stand");
}

#[test]
fn without_r31_settings_the_book_stays_as_before() {
    let plain = sample::input();
    let mut with_inactive = sample::input();
    with_inactive.back_matter = BackMatter { registers: builtin_registers(), ..BackMatter::default() };
    let a = package(&build(&plain).unwrap()).unwrap();
    let b = package(&build(&with_inactive).unwrap()).unwrap();
    assert_eq!(a, b, "inaktives back_matter: dieselben Bytes wie ohne");
    let book = build(&plain).unwrap();
    assert!(
        !book.items.iter().any(|i| matches!(i.kind, ItemKind::Glossary | ItemKind::Index)),
        "keine Register- oder Glossardateien"
    );
}
