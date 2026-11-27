// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 31b: index pages and glossary page measured in the typeset book. The source is built as in the store
//! (`snapshot::export_parts`): work template, rule block [`rules`], per section the projection
//! [`Projector::project`], after that the back matter [`back_matter`]. Measured with
//! [`baruch_render::page_texts`]: on which page (by the printed page number in the footer) is the marked word,
//! and what is in the index? All texts freely invented.

use baruch_render::{PlacedText, Severity, Source, compile, page_texts};
use baruch_typst::index::{
    BackMatter, GlossaryEntry, IndexMark, Projector, Register, RegisterKind, back_matter, builtin_registers,
    mark_source,
};
use baruch_typst::typeset::*;

const PT_PER_MM: f64 = 72.0 / 25.4;

/// Small page, so that little text fills several pages; without hyphenation (words stay whole text pieces).
const TEMPLATE: &str = "#set page(width: 110mm, height: 150mm, margin: 15mm)\n#set text(hyphenate: false)\n";
const PAGE_HEIGHT_PT: f64 = 150.0 * PT_PER_MM;
const PAGE_WIDTH_PT: f64 = 110.0 * PT_PER_MM;

/// Freely invented sentences (without the keywords of the test).
const SENTENCES: [&str; 6] = [
    "Der Gärtner zählte am Morgen die Beete und schrieb jede neue Knospe in sein kleines Heft.",
    "Über dem Zaun hingen Bohnenranken, die der Wind in der Nacht zu einem grünen Knoten gedreht hatte.",
    "Seine Nachbarin brachte Kaffee in einer Kanne mit abgeschlagenem Henkel und blieb eine Weile stehen.",
    "Sie sprachen über den Regen, der seit drei Wochen ausblieb, und über die Tauben auf dem Kirchturm.",
    "Am Nachmittag kamen Kinder und fragten, ob die Kürbisse bis zum Herbst wirklich so groß würden.",
    "Abends saß er auf der Bank vor dem Schuppen und hörte den Amseln zu, bis es dunkel wurde.",
];

fn filler(seed: usize) -> String {
    (0..5).map(|i| SENTENCES[(seed + i) % SENTENCES.len()]).collect::<Vec<_>>().join(" ")
}

fn mark(register: &str, term: &str) -> String {
    mark_source(&IndexMark::new(register, term))
}

fn mark_with(register: &str, term: &str, f: impl FnOnce(&mut IndexMark)) -> String {
    let mut m = IndexMark::new(register, term);
    f(&mut m);
    mark_source(&m)
}

/// Sections of the test book: preamble without chapter (one page), then three chapters with marks.
fn sections() -> Vec<String> {
    let zeder =
        |n: usize| format!("Im Schuppen lag Zedernholz{} vom Vorjahr. {}", mark("stichwort", "Zedernholz"), filler(n));
    let front = format!("Vorspann ohne Kapitel. {}\n\n{}\n\n#pagebreak()\n", filler(0), filler(1));
    let one = format!(
        "= Frühling\n\n{}\n\n{}\n\n{}\n\nAm Abend{} blühte eine Ähre{} am Weg. {}\n\n{}\n\n{}\n\n{}\n",
        zeder(2),
        filler(3),
        filler(4),
        mark("stichwort", "Abend"),
        mark("stichwort", "Ähre"),
        filler(5),
        zeder(0),
        filler(1),
        zeder(2),
    );
    let two = format!(
        "= Sommer\n\nIm Garten{} wuchs das Beet{} am Zaun{}. Noomi{} kam aus Bethlehem{}. {}\n\n\
         Die Huld{} der Nachbarin war groß, und die Gnade{} auch. {}\n\n\
         Wie in Röm 8,28 und Joh 3,16 steht, ist alles gut. // Joh 1,1 im Kommentar zählt nicht\n\n\
         Raw zählt nicht: `Ps 23`. Und 1. Mose 1,1 steht vorn. {}\n",
        mark("stichwort", "Garten"),
        mark_with("stichwort", "Garten", |m| m.sub = "Beet".into()),
        mark_with("stichwort", "Garten", |m| m.sub = "Zaun".into()),
        mark("personen", "Noomi"),
        mark("orte", "Bethlehem"),
        filler(3),
        mark_with("stichwort", "Huld", |m| m.see = "Gnade".into()),
        mark_with("stichwort", "Gnade", |m| m.also = "Liebe".into()),
        filler(4),
        filler(5),
    );
    let three = format!("= Herbst\n\n{}\n\n{}\n", zeder(4), filler(0));
    vec![front, one, two, three]
}

fn settings(columns: u8, registers: &[&str], glossary: Option<GlossaryPosition>) -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::NewPage),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomCenter,
            start: PageNumberStart::FirstChapter,
        }),
        index_pages: (!registers.is_empty())
            .then(|| IndexPages { registers: registers.iter().map(|s| s.to_string()).collect(), columns }),
        glossary_page: glossary,
        ..TypesetSettings::default()
    }
}

fn back(settings: TypesetSettings, glossary: Vec<GlossaryEntry>) -> BackMatter {
    let mut registers = builtin_registers();
    registers.push(Register { slug: "orte".into(), title: "Ortsregister".into(), kind: RegisterKind::Custom });
    BackMatter { settings, registers, glossary }
}

/// Source as in the store: template, rule block, projected sections, back matter.
fn book(back: &BackMatter, sections: &[String]) -> String {
    let mut projector = Projector::new(back);
    let bodies: Vec<String> = sections.iter().map(|s| projector.project(s).into_owned()).collect();
    let tail = back_matter(back, &bodies.iter().map(String::as_str).collect::<Vec<_>>());
    format!("{TEMPLATE}{}{}{tail}", rules(&back.settings), bodies.concat())
}

fn set(main: &str) -> Vec<Vec<PlacedText>> {
    let source = Source { main: main.to_owned(), files: Vec::new() };
    let rendered = compile(&source);
    let errors: Vec<&str> =
        rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.as_str()).collect();
    assert!(errors.is_empty(), "Buch mit Buchende setzt ohne Fehler: {errors:?}\n{main}");
    page_texts(&source).expect("ohne Satzfehler liefert page_texts die Seiten")
}

/// Printed page number of a page: digits in the footer (below the type area).
fn folio(page: &[PlacedText]) -> Option<u32> {
    page.iter()
        .filter(|t| t.y_pt > PAGE_HEIGHT_PT - 15.0 * PT_PER_MM && !t.text.is_empty())
        .find_map(|t| t.text.trim().parse().ok())
}

/// Lines of a page (text pieces with the same baseline, by x), per line (x of the first piece, text with
/// collapsed whitespace).
fn lines(page: &[PlacedText]) -> Vec<(f64, String)> {
    let mut ys: Vec<i64> = page.iter().map(|t| (t.y_pt * 10.0).round() as i64).collect();
    ys.sort_unstable();
    ys.dedup();
    let mut out = Vec::new();
    for y in ys {
        // Multi-column: separate pieces of one baseline by columns (gap above 20 pt).
        let mut row: Vec<&PlacedText> = page.iter().filter(|t| (t.y_pt * 10.0).round() as i64 == y).collect();
        row.sort_by(|a, b| a.x_pt.total_cmp(&b.x_pt));
        let mut current: Vec<&PlacedText> = Vec::new();
        for t in row {
            if let Some(last) = current.last()
                && t.x_pt - (last.x_pt + last.width_pt) > 20.0
            {
                out.push(join(&current));
                current.clear();
            }
            current.push(t);
        }
        if !current.is_empty() {
            out.push(join(&current));
        }
    }
    out
}

fn join(pieces: &[&PlacedText]) -> (f64, String) {
    let text = pieces.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    (pieces[0].x_pt, text.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// First page (index) on which a line reads exactly `heading`.
fn page_with_line(pages: &[Vec<PlacedText>], heading: &str) -> usize {
    pages
        .iter()
        .position(|p| lines(p).iter().any(|(_, l)| l == heading))
        .unwrap_or_else(|| panic!("Zeile {heading:?} fehlt im Satz"))
}

/// Printed page numbers of the pages before `before` on which `word` is.
fn folios_of(pages: &[Vec<PlacedText>], word: &str, before: usize) -> Vec<u32> {
    let mut out: Vec<u32> = pages[..before]
        .iter()
        .filter(|p| {
            p.iter().any(|t| t.text.split_whitespace().any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == word))
        })
        .map(|p| folio(p).expect("Textseite mit Seitenzahl"))
        .collect();
    out.dedup();
    out
}

/// Page ranges as expected in the index: consecutive pages to "a–b".
fn ranges(pages: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < pages.len() {
        let a = pages[i];
        let mut b = a;
        while i + 1 < pages.len() && pages[i + 1] == b + 1 {
            i += 1;
            b = pages[i];
        }
        out.push(if b > a { format!("{a}–{b}") } else { a.to_string() });
        i += 1;
    }
    out.join(", ")
}

/// Lines of the pages from `from` up to before `to` (index pages), without footer.
fn register_lines(pages: &[Vec<PlacedText>], from: usize, to: usize) -> Vec<(f64, String)> {
    pages[from..to]
        .iter()
        .flat_map(|p| {
            let body: Vec<PlacedText> =
                p.iter().filter(|t| t.y_pt <= PAGE_HEIGHT_PT - 15.0 * PT_PER_MM).cloned().collect();
            lines(&body)
        })
        .collect()
}

#[test]
fn keyword_register_prints_measured_pages_ranges_subentries_and_references() {
    let back = back(settings(1, &["stichwort"], None), Vec::new());
    let main = book(&back, &sections());
    let pages = set(&main);
    let index = page_with_line(&pages, "Stichwortregister");
    let lines = register_lines(&pages, index, pages.len());
    let texts: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();

    // Zedernholz is in chapter 1 three times and in chapter 3 once; the printed page numbers (counted from the
    // first chapter, the preamble has no number) are determined by the typesetting.
    let zeder = folios_of(&pages, "Zedernholz", index);
    assert!(zeder.len() >= 3, "Zedernholz steht auf mindestens drei Seiten (Messung): {zeder:?}");
    assert!(zeder.windows(2).any(|w| w[1] == w[0] + 1), "mindestens zwei Folgeseiten für den Bereich: {zeder:?}");
    assert_eq!(zeder[0], 1, "die erste Kapitelseite trägt die 1 (Zählung ab dem ersten Kapitel): {zeder:?}");
    let expected = format!("Zedernholz {}", ranges(&zeder));
    assert!(expected.contains('–'), "Folgeseiten werden zum Bereich zusammengefasst: {expected}");
    assert!(texts.contains(&expected.as_str()), "Registerzeile mit gemessenen Seiten {expected:?}: {texts:?}");

    // German sorting (DIN 5007-1): Ähre like Ahre, so between Abend and Garten; group A.
    let pos = |prefix: &str| {
        texts.iter().position(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix} fehlt: {texts:?}"))
    };
    assert!(pos("Abend") < pos("Ähre") && pos("Ähre") < pos("Garten"), "Ä bei A einsortiert: {texts:?}");
    assert!(pos("A") < pos("Abend") && texts[pos("A")] == "A", "Buchstabe A als Gruppenkopf vor Abend: {texts:?}");

    // Subheadwords indented under their headword.
    let garten = &lines[pos("Garten")];
    let beet = &lines[pos("Beet")];
    let zaun = &lines[pos("Zaun")];
    assert!(pos("Garten") < pos("Beet") && pos("Beet") < pos("Zaun"), "Unterstichwörter sortiert darunter: {texts:?}");
    assert!(beet.0 > garten.0 + 5.0, "Unterstichwort eingerückt: Garten {:.1} pt, Beet {:.1} pt", garten.0, beet.0);
    assert!((beet.0 - zaun.0).abs() < 0.5, "beide Unterstichwörter gleich eingerückt");

    // „siehe“ without page number, „siehe auch“ after the pages.
    assert!(texts.contains(&"Huld siehe Gnade"), "„siehe“-Eintrag ohne Seitenzahl: {texts:?}");
    let gnade = texts[pos("Gnade")];
    assert!(
        gnade.starts_with("Gnade ") && gnade.ends_with("; siehe auch Liebe"),
        "„siehe auch“ hinter der Seitenzahl: {gnade}"
    );
    assert!(gnade.chars().nth(6).is_some_and(|c| c.is_ascii_digit()), "Gnade trägt eine Seitenzahl: {gnade}");
}

#[test]
fn scripture_register_lists_recognized_passages_in_biblical_order() {
    let back = back(settings(1, &["bibel"], None), Vec::new());
    let main = book(&back, &sections());
    let pages = set(&main);
    let index = page_with_line(&pages, "Bibelstellenregister");
    let texts: Vec<String> = register_lines(&pages, index, pages.len()).into_iter().map(|(_, l)| l).collect();
    let page = folios_of(&pages, "Röm", index);
    assert_eq!(page.len(), 1, "alle Stellen stehen in einem Absatz von Kapitel 2: {page:?}");
    let p = page[0];
    let at = |line: &str| texts.iter().position(|l| l == line).unwrap_or_else(|| panic!("{line:?} fehlt: {texts:?}"));
    // Biblical order (not alphabetical: „Johannes“ before „Römer“, „1. Mose“ first).
    assert!(at("1. Mose") < at("Johannes") && at("Johannes") < at("Römer"), "Bücher in Bibelordnung: {texts:?}");
    assert!(at(&format!("1,1 {p}")) == at("1. Mose") + 1, "Stelle 1,1 unter 1. Mose mit Seite {p}: {texts:?}");
    assert!(at(&format!("3,16 {p}")) == at("Johannes") + 1, "Joh 3,16 mit Seite {p}: {texts:?}");
    assert!(at(&format!("8,28 {p}")) == at("Römer") + 1, "Röm 8,28 mit Seite {p}: {texts:?}");
    assert!(!texts.iter().any(|l| l.starts_with("Psalm")), "Ps 23 in Raw zählt nicht: {texts:?}");
    assert_eq!(texts.iter().filter(|l| l.starts_with("1,1")).count(), 1, "Joh 1,1 im Kommentar zählt nicht: {texts:?}");
}

#[test]
fn person_and_custom_registers_get_their_own_pages_in_two_columns() {
    let back = back(settings(2, &["personen", "orte", "stichwort"], None), Vec::new());
    // Many headwords, so that the index fills more than one column.
    let mut many = sections();
    let words: Vec<String> =
        (0..80).map(|i| format!("Vogel{i:02}{}", mark("stichwort", &format!("Vogel {i:02}")))).collect();
    many.push(format!("= Winter\n\n{}\n", words.join(" ")));
    let main = book(&back, &many);
    let pages = set(&main);
    let persons = page_with_line(&pages, "Personenregister");
    let places = page_with_line(&pages, "Ortsregister");
    let keywords = page_with_line(&pages, "Stichwortregister");
    assert!(persons < places && places < keywords, "Reihenfolge der Einstellung: {persons} {places} {keywords}");
    let noomi = folios_of(&pages, "Noomi", persons)[0];
    let person_lines: Vec<String> = register_lines(&pages, persons, places).into_iter().map(|(_, l)| l).collect();
    assert!(person_lines.contains(&format!("Noomi {noomi}")), "Personenregister mit Seite {noomi}: {person_lines:?}");
    let place_lines: Vec<String> = register_lines(&pages, places, keywords).into_iter().map(|(_, l)| l).collect();
    let bethlehem = folios_of(&pages, "Bethlehem", persons)[0];
    assert!(place_lines.contains(&format!("Bethlehem {bethlehem}")), "eigenes Register (Orte): {place_lines:?}");
    // Two columns: entries are to the right of the page centre.
    let keyword_lines = register_lines(&pages, keywords, pages.len());
    assert!(
        keyword_lines.iter().any(|(x, _)| *x > PAGE_WIDTH_PT / 2.0),
        "mit columns 2 steht ein Teil der Einträge in der rechten Spalte: {keyword_lines:?}"
    );
    let one_column = book(&back_with_columns(1), &many);
    let pages = set(&one_column);
    let keywords = page_with_line(&pages, "Stichwortregister");
    assert!(
        register_lines(&pages, keywords, pages.len()).iter().all(|(x, _)| *x < PAGE_WIDTH_PT / 2.0),
        "mit columns 1 steht alles in einer Spalte"
    );
}

fn back_with_columns(columns: u8) -> BackMatter {
    back(settings(columns, &["stichwort"], None), Vec::new())
}

fn glossary() -> Vec<GlossaryEntry> {
    vec![GlossaryEntry {
        uuid: "3f1c2a4e-0000-4000-8000-000000000031".into(),
        term: "Zedernholz".into(),
        variants: vec!["Zeder".into()],
        short: "Duftendes Bauholz.".into(),
        long: "Es hält lange.\n\nMan riecht es noch nach Jahren.".into(),
        bible_refs: vec!["1. Kön 5,20".into()],
    }]
}

#[test]
fn glossary_page_sits_before_or_after_the_registers_and_first_mentions_link_to_it() {
    for (position, before) in [(GlossaryPosition::BeforeIndexes, true), (GlossaryPosition::AfterIndexes, false)] {
        let mut s = settings(1, &["stichwort"], Some(position));
        s.glossary_first_mention = Some(true);
        let back = back(s, glossary());
        let main = book(&back, &sections());
        assert!(
            main.contains("#link(<baruch-gl-3f1c2a4e-0000-4000-8000-000000000031>)[#emph[Zedernholz]]"),
            "erste Erwähnung als Link auf das Label der Glossarseite"
        );
        let pages = set(&main);
        let gloss = page_with_line(&pages, "Glossar");
        let index = page_with_line(&pages, "Stichwortregister");
        assert_eq!(gloss < index, before, "{position:?}: Glossarseite {gloss}, Register {index}");
        let end = if before { index } else { pages.len() };
        let texts: Vec<String> = register_lines(&pages, gloss, end).into_iter().map(|(_, l)| l).collect();
        assert!(
            texts.iter().any(|l| l == "Zedernholz (Zeder) – Duftendes Bauholz."),
            "Begriff, Variante und kurze Erklärung: {texts:?}"
        );
        assert!(texts.iter().any(|l| l == "Es hält lange."), "ausführliche Erklärung, erster Absatz: {texts:?}");
        assert!(texts.iter().any(|l| l == "Man riecht es noch nach Jahren."), "zweiter Absatz: {texts:?}");
        assert!(texts.iter().any(|l| l == "Bibelstellen: 1. Kön 5,20"), "Bibelstellen: {texts:?}");
    }
}

#[test]
fn first_mention_without_glossary_page_sets_without_a_dead_label() {
    let mut s = settings(1, &[], None);
    s.glossary_first_mention = Some(true);
    let back = back(s, glossary());
    let main = book(&back, &sections());
    assert!(!main.contains("baruch-gl-"), "ohne Glossarseite kein Verweis auf ein Label");
    assert!(main.contains("#emph[Zedernholz]"), "erste Erwähnung hervorgehoben");
    assert_eq!(main.matches("#emph[Zedernholz]").count(), 2, "je Kapitel einmal (Kapitel 1 und 3)");
    // `set` checks that the book typesets without errors (no dead label reference).
    let pages = set(&main);
    assert!(!pages.is_empty(), "gesetzt");
}

#[test]
fn the_guard_semicolon_and_marks_leave_no_trace_in_the_set_text() {
    let mut s = settings(1, &[], None);
    s.glossary_first_mention = Some(true);
    let back = back(s, glossary());
    let body =
        format!("= Kapitel\n\nZedernholz(Probe) liegt da. Danach{} kommt nichts.\n", mark("stichwort", "Danach"));
    let main = book(&back, &[body]);
    assert!(main.contains("#emph[Zedernholz];(Probe)"), "Projektion setzt das Semikolon vor die Klammer: {main}");
    let pages = set(&main);
    let text: Vec<String> = pages.iter().flat_map(|p| lines(p)).map(|(_, l)| l).collect();
    assert!(
        text.iter().any(|l| l.replace(' ', "") == "Zedernholz(Probe)liegtda.Danachkommtnichts."),
        "Semikolon und Marke erscheinen nicht im Satz, die Klammer folgt dem Wort: {text:?}"
    );
    assert!(!text.iter().any(|l| l.contains(';')), "kein Semikolon im gesetzten Text: {text:?}");
}
