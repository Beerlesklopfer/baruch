// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b (ausgaben, contract §4.1): golden German outputs of `baruch-typst`. The fixtures under
//! `tests/data/round39b/` were generated with the unchanged S1 code (commit 5c5a821) before any work-language change;
//! every later change must keep them byte-equal, because German works must not change.
//!
//! Regenerating them is forbidden after the first commit of the round. The environment variable `BARUCH_BLESS_R39B`
//! only exists for that first commit (it writes the files instead of comparing).
//!
//! Covered: the numbering rule, part and heading numbers of every scheme, the exported template, the back matter
//! (index pages and glossary, empty index), imprint and cover/front-matter Typst, the cross-reference snippet with
//! page, the draft watermark and the typesetting rule block with a language. Book details freely invented.

use std::path::PathBuf;

use baruch_typst::cover::{
    BookInfo, CoverRef, FrameOptions, TitleArea, WrapOptions, book_dict, frame_parts, front_source_with_rules,
    imprint_lines, wrap_source_with_rules,
};
use baruch_typst::index::{
    BackMatter, GlossaryEntry, IndexMark, Projector, back_matter, builtin_registers, mark_source,
};
use baruch_typst::scheme::{Numbering, Scheme, export_template, ordinal_word};
use baruch_typst::typeset::{Draft, GlossaryPosition, IndexPages, TypesetSettings, draft_line, draft_rule, rules};
use baruch_typst::xref::xref_snippet;

/// Compares `actual` with the fixture `name` (or writes it while blessing the first commit).
pub fn golden(name: &str, actual: &str) {
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

/// Number of every scheme: rule, spec, parts 1–3, headings, export template.
fn numbering_report(numbering_of: impl Fn(Scheme) -> Numbering) -> String {
    let mut out = String::new();
    for scheme in Scheme::ALL {
        let numbering = numbering_of(scheme);
        out.push_str(&format!("== {}\n", scheme.id()));
        out.push_str(&format!("rule: {}", numbering.heading_rule()));
        out.push_str(&format!("spec: {:?}\n", numbering.spec()));
        for part in 1..=3 {
            out.push_str(&format!("part {part}: {}\n", numbering.part(part)));
        }
        for path in [&[1][..], &[2], &[3], &[4], &[2, 1], &[2, 1, 3], &[21], &[100]] {
            out.push_str(&format!("heading {path:?} of 4: {}\n", numbering.heading(path, 4)));
        }
        out.push_str("template:\n");
        out.push_str(&export_template("#set text(lang: \"de\")\n", scheme, &numbering));
    }
    out
}

#[test]
fn the_numbering_of_every_scheme_stays_german_for_german_works() {
    let report = numbering_report(|scheme| {
        Numbering::of_scheme(scheme, Some("I.;A.;1."), "#set heading(numbering: \"1.a\")\n")
            .expect("every scheme has a numbering with this custom pattern and template")
    });
    let words = format!(
        "{}\n{}\n{}\n",
        ordinal_word(1, "es", "Kapitel"),
        ordinal_word(21, "er", "Teil"),
        ordinal_word(100, "es", "Kapitel")
    );
    golden("numbering.de.txt", &format!("{report}== ordinal words\n{words}"));
}

fn glossary() -> Vec<GlossaryEntry> {
    vec![
        GlossaryEntry {
            uuid: "7b0e5c1d-0000-4000-8000-000000000039".into(),
            term: "Weide".into(),
            variants: vec!["Salix".into()],
            short: "Baum am Wasser.".into(),
            long: "Sie wächst schnell.\n\nIhre Zweige biegen sich.".into(),
            bible_refs: vec!["Ps 137,2".into()],
        },
        GlossaryEntry {
            uuid: "7b0e5c1d-0000-4000-8000-000000000040".into(),
            term: "Lehm".into(),
            short: "Schwerer Boden.".into(),
            ..GlossaryEntry::default()
        },
    ]
}

fn back(glossary_page: GlossaryPosition, registers: &[&str]) -> BackMatter {
    BackMatter {
        settings: TypesetSettings {
            index_pages: Some(IndexPages { registers: registers.iter().map(|r| r.to_string()).collect(), columns: 2 }),
            glossary_page: Some(glossary_page),
            glossary_first_mention: Some(true),
            ..TypesetSettings::default()
        },
        registers: builtin_registers(),
        glossary: glossary(),
        ..BackMatter::default()
    }
}

#[test]
fn the_back_matter_stays_german_for_german_works() {
    let lehm = mark_source(&IndexMark::new("stichwort", "Lehm"));
    let sections = [
        format!("= Der Platz\n\nAm Bach steht eine Weide, wie Ps 46,4 sagt.{lehm}\n"),
        "= Das Fundament\n\nDer Lehm trägt schlecht.\n".to_owned(),
    ];
    let mut out = String::new();
    for (name, back) in [
        ("before", back(GlossaryPosition::BeforeIndexes, &["stichwort", "bibel", "personen"])),
        ("after", back(GlossaryPosition::AfterIndexes, &["personen"])),
    ] {
        let mut projector = Projector::new(&back);
        let projected: Vec<String> = sections.iter().map(|s| projector.project(s).into_owned()).collect();
        out.push_str(&format!("== {name}\n"));
        for section in &projected {
            out.push_str(section);
        }
        out.push_str(&back_matter(&back, &projected.iter().map(String::as_str).collect::<Vec<_>>()));
    }
    golden("back_matter.de.typ", &out);
}

fn info() -> BookInfo {
    BookInfo {
        title: "Das Gartenhaus am Bach".into(),
        subtitle: "Ein Bautagebuch".into(),
        author: "Erika Beispiel".into(),
        publisher: "Beispielverlag".into(),
        place: "Musterstadt".into(),
        year: "2026".into(),
        edition: "1. Auflage".into(),
        isbn: "978-3-16-148410-0".into(),
        price: "12,00 €".into(),
        series: "Bauen am Wasser".into(),
        printer: "Druckerei Muster".into(),
        bible: "Erfundene Übersetzung".into(),
        bible_notice: "Mit freundlicher Genehmigung.".into(),
        image_credit: "Zeichnungen: Erika Beispiel".into(),
        dedication: "Für alle, die am Bach bauen.".into(),
        blurb: "Wie aus einer Weide am Bach ein Gartenhaus wurde.".into(),
        bio: "Erika Beispiel baut gern.".into(),
        publisher_address: "Weg 1\n12345 Musterstadt".into(),
        printer_address: "Druckweg 2\n12345 Musterstadt".into(),
    }
}

fn wrap(cover: bool, barcode: bool) -> WrapOptions {
    WrapOptions {
        page_width_mm: 120.0,
        page_height_mm: 180.0,
        spine_mm: 9.0,
        bleed_mm: 3.0,
        title_area: TitleArea::Middle,
        cover: cover.then(|| CoverRef { path: ".baruch/umschlag.png".into(), alt: "Weide".into() }),
        barcode: barcode.then(|| ".baruch/isbn.svg".into()),
        author_photo: None,
        layout: None,
    }
}

#[test]
fn imprint_cover_and_front_matter_stay_german_for_german_works() {
    let mut out = String::from("== imprint lines\n");
    for line in imprint_lines(&info()) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("== imprint lines (bare)\n");
    for line in imprint_lines(&BookInfo { title: "Nur ein Titel".into(), ..BookInfo::default() }) {
        out.push_str(&line);
        out.push('\n');
    }
    let frame = frame_parts(
        &info(),
        &FrameOptions {
            cover: Some(CoverRef { path: ".baruch/umschlag.png".into(), alt: "Weide".into() }),
            cover_pages: true,
            front_matter: true,
            title_area: TitleArea::Top,
            barcode: Some(".baruch/isbn.svg".into()),
            author_photo: None,
            layout: None,
            template_functions: Default::default(),
        },
    );
    out.push_str(&format!("== frame front\n{}\n== frame back\n{}\n", frame.front, frame.back));
    out.push_str(&format!("== wrap with barcode\n{}\n", wrap_source_with_rules(&info(), &wrap(true, true), "")));
    let without_isbn = BookInfo { isbn: String::new(), ..info() };
    out.push_str(&format!(
        "== wrap without isbn\n{}\n",
        wrap_source_with_rules(&without_isbn, &wrap(false, false), "")
    ));
    out.push_str(&format!("== front\n{}\n", front_source_with_rules(&info(), &wrap(true, false), "")));
    out.push_str(&format!("== book dict\n{}\n", book_dict(&info())));
    golden("cover.de.typ", &out);
}

#[test]
fn cross_references_draft_and_rules_stay_german_for_german_works() {
    let mut out = String::new();
    out.push_str(&format!("{}\n{}\n", xref_snippet(103, "die Einleitung", true), xref_snippet(103, "x", false)));
    let draft = Draft { date: true, note: "Fassung für das Lektorat".into(), ..Draft::default() };
    out.push_str(&format!("{}\n", draft_line(&draft, Some("9.11.2026"))));
    out.push_str(&draft_rule(&draft, Some("9.11.2026")));
    out.push_str(&draft_rule(&Draft::default(), None));
    out.push_str(&rules(&TypesetSettings { lang: Some("de-CH".into()), hyphenate: Some(true), ..Default::default() }));
    golden("typst_misc.de.typ", &out);
}
