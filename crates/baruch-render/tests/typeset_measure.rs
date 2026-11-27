// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 29b: every typesetting setting measured in the typeset book (contract §5.2/§8.2). The source is built
//! as in the server: work template, then the rule block [`rules`] (hook in `snapshot::head`), then – with parts
//! – the rule of the part pages [`part_page_rule_for`] (like `typeset::prepare`), then the text. Measured with
//! [`baruch_render::page_texts`] (baseline left in pt from the top left) and the page dimensions of the SVG
//! pages. All texts freely invented.

use baruch_render::{PlacedText, Rendered, Severity, Source, compile, page_sizes_pt, page_texts};
use baruch_typst::typeset::*;
use baruch_typst::{part_line, xref};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn mm(x: f64) -> f64 {
    x * PT_PER_MM
}

/// Freely invented sentences.
const SENTENCES: [&str; 8] = [
    "Der Gärtner zählte am Morgen die Beete und schrieb jede neue Knospe in sein kleines Heft.",
    "Über dem Zaun hingen Bohnenranken, die der Wind in der Nacht zu einem grünen Knoten gedreht hatte.",
    "Seine Nachbarin brachte Kaffee in einer Kanne mit abgeschlagenem Henkel und blieb eine Weile stehen.",
    "Sie sprachen über den Regen, der seit drei Wochen ausblieb, und über die Tauben auf dem Kirchturm.",
    "Am Nachmittag kamen Kinder und fragten, ob die Kürbisse bis zum Herbst wirklich so groß würden.",
    "Der Gärtner lachte und zeigte ihnen einen Kürbis, der schon jetzt größer war als eine Gießkanne.",
    "Abends saß er auf der Bank vor dem Schuppen und hörte den Amseln zu, bis es dunkel wurde.",
    "Dann trug er die Gießkanne zurück, schloss das Tor und ging langsam den Kiesweg hinunter.",
];

/// A paragraph of `n` sentences from sentence `seed`.
fn paragraph(seed: usize, n: usize) -> String {
    (0..n).map(|i| SENTENCES[(seed + i) % SENTENCES.len()]).collect::<Vec<_>>().join(" ")
}

/// A chapter with `paragraphs` paragraphs of six sentences each.
fn chapter(title: &str, paragraphs: usize) -> String {
    let mut out = format!("= {title}\n\n");
    for p in 0..paragraphs {
        out.push_str(&paragraph(p * 3, 6));
        out.push_str("\n\n");
    }
    out
}

/// Source as in the server: template, rule block, part rule (only with parts), text.
fn book(template: &str, s: &TypesetSettings, body: &str) -> String {
    let parts = body.matches("<baruch-part>").count();
    let part_rule = if parts > 0 { part_page_rule_for(&vec![String::new(); parts], s) } else { String::new() };
    format!("{template}\n{}{part_rule}{body}", rules(s))
}

fn source(main: &str) -> Source {
    Source { main: main.to_owned(), files: Vec::new() }
}

/// Typeset without errors: typesetting result (pages, headings) and text pieces per page.
fn set(main: &str) -> (Rendered, Vec<Vec<PlacedText>>) {
    let rendered = compile(&source(main));
    let errors: Vec<&str> =
        rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.as_str()).collect();
    assert!(errors.is_empty(), "die Regeln müssen ohne Fehler setzen: {errors:?}\n{main}");
    let texts = page_texts(&source(main)).expect("ohne Satzfehler liefert page_texts die Seiten");
    assert_eq!(texts.len(), rendered.pages.len(), "page_texts und compile zählen dieselben Seiten");
    (rendered, texts)
}

/// Page (1-based) of the heading whose line starts with `prefix`.
fn heading_page(rendered: &Rendered, main: &str, prefix: &str) -> usize {
    let line = main.lines().position(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix:?} fehlt")) + 1;
    rendered.headings.iter().find(|h| h.line == line).unwrap_or_else(|| panic!("Überschrift {prefix:?} ungesetzt")).page
}

/// Lines of a page: text pieces with the same baseline (to 0.1 pt), from top to bottom.
fn lines(page: &[PlacedText]) -> Vec<Vec<&PlacedText>> {
    let mut ys: Vec<i64> = page.iter().map(|t| (t.y_pt * 10.0).round() as i64).collect();
    ys.sort_unstable();
    ys.dedup();
    ys.into_iter()
        .map(|y| {
            let mut line: Vec<&PlacedText> = page.iter().filter(|t| (t.y_pt * 10.0).round() as i64 == y).collect();
            line.sort_by(|a, b| a.x_pt.total_cmp(&b.x_pt));
            line
        })
        .collect()
}

fn line_text(line: &[&PlacedText]) -> String {
    line.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("")
}

/// Text pieces in the type area (between `top` and `bottom` pt).
fn body(page: &[PlacedText], top: f64, bottom: f64) -> Vec<&PlacedText> {
    page.iter().filter(|t| t.y_pt > top && t.y_pt < bottom).collect()
}

/// Empty page: no text piece in the type area. Not `page.is_empty()` – an empty page that wrongly carries a
/// page number or running head is never entirely empty and would otherwise pass as a text page (finding R29).
fn blank(page: &[PlacedText], top: f64, bottom: f64) -> bool {
    body(page, top, bottom).is_empty()
}

fn a5() -> (f64, f64) {
    (mm(148.0), mm(210.0))
}

/// A5 with margins 20/15/18/22 and 5 mm gutter (contract example).
fn a5_margins() -> TypesetSettings {
    TypesetSettings {
        page_format: Some(PageFormat::A5),
        margins: Some(Margins { inside_mm: 20.0, outside_mm: 15.0, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 5.0 }),
        ..Default::default()
    }
}

// ---- Page ----------------------------------------------------------------------------------------

#[test]
fn every_page_format_has_its_size_within_half_a_point() {
    let formats = PageFormat::ALL.iter().copied().chain([PageFormat::Custom { width_mm: 100.0, height_mm: 160.0 }]);
    for format in formats {
        let s = TypesetSettings { page_format: Some(format), ..Default::default() };
        // The work template sets A4; the setting comes after it and applies (conflict "overrides").
        let main = book("#set page(paper: \"a4\")", &s, &chapter("Format", 1));
        let (rendered, _) = set(&main);
        let (w, h) = format.size_mm();
        for (i, (pw, ph)) in page_sizes_pt(&rendered).into_iter().enumerate() {
            assert!(
                (pw - mm(w)).abs() <= 0.5 && (ph - mm(h)).abs() <= 0.5,
                "{format:?}: Seite {} misst {pw:.2} × {ph:.2} pt, erwartet {:.2} × {:.2} pt",
                i + 1,
                mm(w),
                mm(h)
            );
        }
    }
}

#[test]
fn margins_add_the_binding_inside_and_mirror_on_even_pages() {
    let s = TypesetSettings { justify: Some(true), ..a5_margins() };
    let main = book("", &s, &chapter("Ränder", 8));
    let (_, pages) = set(&main);
    assert!(pages.len() >= 3, "genug Text für ungerade und gerade Seiten: {}", pages.len());
    let (w, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    for (i, page) in pages.iter().enumerate() {
        let n = i + 1;
        let text = body(page, top, bottom);
        let left = text.iter().map(|t| t.x_pt).fold(f64::MAX, f64::min);
        // Right edge only from lines that end with a letter (punctuation marks hang into the margin).
        let right = lines(page)
            .iter()
            .filter(|l| l[0].y_pt > top && l[0].y_pt < bottom)
            .filter(|l| line_text(l).trim_end().chars().last().is_some_and(char::is_alphabetic))
            .map(|l| l.iter().map(|t| t.x_pt + t.width_pt).fold(f64::MIN, f64::max))
            .fold(f64::MIN, f64::max);
        // Odd page = right page: inside (binding) on the left. Inner margin = 20 mm + 5 mm gutter.
        let (want_left, want_right) = if n % 2 == 1 { (mm(25.0), w - mm(15.0)) } else { (mm(15.0), w - mm(25.0)) };
        assert!((left - want_left).abs() <= 0.5, "Seite {n}: linker Rand {left:.2} pt, erwartet {want_left:.2} pt");
        assert!(
            (right - want_right).abs() <= 0.6,
            "Seite {n}: rechte Satzkante {right:.2} pt (Blocksatz), erwartet {want_right:.2} pt"
        );
        let first = text.iter().map(|t| t.y_pt).fold(f64::MAX, f64::min);
        let last = text.iter().map(|t| t.y_pt).fold(f64::MIN, f64::max);
        // First baseline: top margin + cap height (< one font size); last one above the bottom margin.
        assert!(first > top && first < top + 14.0, "Seite {n}: erste Grundlinie {first:.2} pt unter {top:.2} pt");
        assert!(last <= bottom + 0.5, "Seite {n}: letzte Grundlinie {last:.2} pt über dem unteren Rand {bottom:.2}");
    }
}

// ---- Chapters and parts --------------------------------------------------------------------------

/// Two short chapters after a preface paragraph; the second chapter starts in the middle of page 1 if nothing
/// breaks.
fn short_book() -> String {
    format!(
        "{}\n\n= Erstes Kapitel\n\n{}\n\n= Zweites Kapitel\n\n{}\n",
        paragraph(0, 3),
        paragraph(2, 3),
        paragraph(4, 3)
    )
}

#[test]
fn chapter_start_continuous_new_page_and_right_page_move_chapters() {
    let measure = |start: Option<ChapterStart>| {
        let s = TypesetSettings { chapter_start: start, page_format: Some(PageFormat::A5), ..Default::default() };
        let main = book("", &s, &short_book());
        let (rendered, pages) = set(&main);
        (
            heading_page(&rendered, &main, "= Erstes"),
            heading_page(&rendered, &main, "= Zweites"),
            pages.iter().map(|p| p.is_empty()).collect::<Vec<bool>>(),
        )
    };
    assert_eq!(measure(Some(ChapterStart::Continuous)).0, 1, "fortlaufend: das erste Kapitel folgt dem Vorwort");
    assert_eq!(measure(Some(ChapterStart::Continuous)).1, 1, "fortlaufend: auch das zweite bleibt auf Seite 1");
    let (first, second, _) = measure(Some(ChapterStart::NewPage));
    assert_eq!((first, second), (2, 3), "neue Seite: jedes Kapitel auf der nächsten Seite");
    let (first, second, empty) = measure(Some(ChapterStart::RightPage));
    assert_eq!((first, second), (3, 5), "neue rechte Seite: Kapitel nur auf ungeraden Seiten");
    assert_eq!(empty, [false, true, false, true, false], "die Leerseiten 2 und 4 tragen keinen Text");
}

#[test]
fn chapter_rule_breaks_before_parts_in_the_export_form() {
    // Export form (without the part rule of the typesetting): part headings are level 1 with <baruch-part> and
    // start on a new page like a chapter (as with the adoptable template rule, finding R29).
    let body = format!(
        "{}\n\n{}\n= Kapitel nach dem Teil\n\n{}\n",
        paragraph(0, 2),
        part_line("Erster Teil"),
        paragraph(3, 2)
    );
    let s = TypesetSettings { chapter_start: Some(ChapterStart::NewPage), ..Default::default() };
    let main = format!("{}{body}", rules(&s));
    let (rendered, _) = set(&main);
    assert_eq!(
        heading_page(&rendered, &main, "#heading(level: 1, numbering: none)[Erster Teil]"),
        2,
        "die Kapitelregel bricht auch vor einem Teil um (er folgt dem Absatz auf Seite 2)"
    );
    assert_eq!(heading_page(&rendered, &main, "= Kapitel nach"), 3, "das Kapitel danach bricht ebenfalls um");
}

/// Finding R29: „Vorlagenregel übernehmen“ loses nothing outside the book typesetting. Export form (work
/// template, rule block, text, no part rule) with two parts and two chapters: before, the canonical rule in the
/// template and no settings, after, an empty template and `chapter_start` right page – all headings on the same
/// pages, the same number of pages. Likewise for "new page".
#[test]
fn adopting_the_template_rule_keeps_every_heading_page_in_the_export_form() {
    let body = format!("{}\n{}{}\n{}", part_line("Eins"), chapter("Alpha", 2), part_line("Zwei"), chapter("Beta", 2));
    let prefixes =
        ["#heading(level: 1, numbering: none)[Eins]", "= Alpha", "#heading(level: 1, numbering: none)[Zwei]", "= Beta"];
    for (template_rule, start) in [
        (CANONICAL_CHAPTER_BREAK, ChapterStart::RightPage),
        ("#show heading.where(level: 1): it => { pagebreak(weak: true); it }", ChapterStart::NewPage),
    ] {
        let found = detect_chapter_break(template_rule).expect("übernehmbar");
        assert_eq!(found.chapter_start, start, "{template_rule}: erkannter Wert");
        let before = format!("{template_rule}\n{body}");
        let after_settings = TypesetSettings { chapter_start: Some(start), ..Default::default() };
        let after = format!("{}\n{}{body}", remove_rule(template_rule, &found), rules(&after_settings));
        let measure = |main: &str| {
            let (rendered, pages) = set(main);
            (prefixes.iter().map(|p| heading_page(&rendered, main, p)).collect::<Vec<usize>>(), pages.len())
        };
        let (pages_before, count_before) = measure(&before);
        let (pages_after, count_after) = measure(&after);
        assert_eq!(pages_after, pages_before, "{start:?}: Überschriften auf denselben Seiten nach der Übernahme");
        assert_eq!(count_after, count_before, "{start:?}: gleich viele Seiten");
        assert!(pages_before[2] > pages_before[1], "{start:?}: der zweite Teil beginnt nach Alpha auf neuer Seite");
    }
}

#[test]
fn part_page_side_and_blank_back_are_honoured() {
    let body = format!("{}\n\n{}\n= Kapitel im Teil\n\n{}\n", paragraph(0, 2), part_line("Vom Säen"), paragraph(1, 2));
    let measure = |part: Option<PartPage>| {
        let s = TypesetSettings { part_page: part, page_format: Some(PageFormat::A5), ..Default::default() };
        let main = book("", &s, &body);
        let (rendered, pages) = set(&main);
        let part_page = heading_page(&rendered, &main, "#heading(level: 1, numbering: none)[Vom Säen]");
        let chapter_page = heading_page(&rendered, &main, "= Kapitel im Teil");
        (part_page, chapter_page, pages.iter().map(|p| p.is_empty()).collect::<Vec<bool>>())
    };
    let (part, chapter, _) = measure(None);
    assert_eq!((part, chapter), (3, 4), "Vorgabe (Runde 14): Teil rechts, Kapitel auf der Rückseite");
    let (part, chapter, empty) = measure(Some(PartPage { side: PartSide::Right, blank_back: true }));
    assert_eq!((part, chapter), (3, 5), "rechts mit leerer Rückseite: Kapitel erst auf der nächsten rechten Seite");
    assert!(empty[3], "die Rückseite (Seite 4) bleibt leer: {empty:?}");
    let (part, chapter, _) = measure(Some(PartPage { side: PartSide::Any, blank_back: false }));
    assert_eq!((part, chapter), (2, 3), "beliebige Seite: Teil auf der nächsten Seite, Kapitel dahinter");
    let (part, chapter, empty) = measure(Some(PartPage { side: PartSide::Any, blank_back: true }));
    assert_eq!((part, chapter), (2, 4), "beliebig mit leerer Rückseite: eine leere Seite zwischen Teil und Kapitel");
    assert!(empty[2], "Seite 3 bleibt leer: {empty:?}");
}

#[test]
fn part_page_rule_without_settings_is_the_runde_14_rule() {
    let labels = vec!["I".to_owned(), String::new()];
    assert_eq!(
        part_page_rule_with(&labels, None),
        baruch_typst::part_page_rule(&labels),
        "ohne Einstellung byte-gleich – ein Werk ohne Satz-Einstellungen setzt wie vor Runde 29"
    );
}

// ---- Font and paragraph --------------------------------------------------------------------------

#[test]
fn font_size_and_leading_are_measured_on_the_baselines() {
    let gap = |leading: f64| {
        let s = TypesetSettings {
            font: Some("DejaVu Sans Mono".into()),
            font_size_pt: Some(12.0),
            leading_em: Some(leading),
            page_format: Some(PageFormat::A5),
            ..Default::default()
        };
        let main = book("", &s, &paragraph(0, 8));
        let (_, pages) = set(&main);
        let rows = lines(&pages[0]);
        let first = rows[0][0];
        assert_eq!(first.font, "DejaVu Sans Mono", "Grundschrift aus der Einstellung");
        assert!((first.size_pt - 12.0).abs() < 0.01, "Schriftgröße 12 pt: {}", first.size_pt);
        rows[1][0].y_pt - rows[0][0].y_pt
    };
    let (narrow, wide) = (gap(0.5), gap(1.0));
    // The leading grows by 0.5 em = 6 pt; the line height of the font stays the same.
    assert!(((wide - narrow) - 6.0).abs() < 0.2, "Grundlinienabstand {narrow:.2} → {wide:.2} pt, erwartet +6 pt");
}

#[test]
fn paragraph_indent_and_spacing_are_measured() {
    let measure = |p: Paragraph| {
        let s = TypesetSettings { paragraph: Some(p), font_size_pt: Some(10.0), ..a5_margins() };
        // The first paragraph follows the heading, the next ones follow each other.
        let main =
            book("", &s, &format!("= Absätze\n\n{}\n\n{}\n\n{}\n", paragraph(0, 2), paragraph(2, 2), paragraph(4, 2)));
        let (_, pages) = set(&main);
        let rows: Vec<Vec<&PlacedText>> =
            lines(&pages[0]).into_iter().filter(|l| l[0].size_pt < 11.0).collect::<Vec<_>>();
        let left = mm(25.0);
        let starts: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                ["Der Gärtner", "Seine Nachbarin", "Am Nachmittag"].iter().any(|w| line_text(l).starts_with(w))
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            starts.len(),
            3,
            "drei Absatzanfänge gefunden: {:?}",
            rows.iter().map(|l| line_text(l)).collect::<Vec<_>>()
        );
        let x: Vec<f64> = starts.iter().map(|&i| rows[i][0].x_pt - left).collect();
        let line_gap = rows[1][0].y_pt - rows[0][0].y_pt;
        let par_gap = rows[starts[1]][0].y_pt - rows[starts[1] - 1][0].y_pt;
        (x, line_gap, par_gap)
    };
    let (x, line_gap, par_gap) = measure(Paragraph::Indent { em: 1.5 });
    assert!(x[0].abs() < 0.5, "nach der Überschrift kein Einzug: {:.2} pt", x[0]);
    assert!((x[1] - 15.0).abs() < 0.5 && (x[2] - 15.0).abs() < 0.5, "Folgeabsätze 1,5 em = 15 pt eingezogen: {x:?}");
    assert!(
        (par_gap - line_gap).abs() < 0.2,
        "Einzug: kein Zwischenraum außer dem Zeilenabstand ({par_gap:.2} vs {line_gap:.2})"
    );
    let (x, line_gap, par_gap) = measure(Paragraph::Spacing { em: 1.2 });
    assert!(x.iter().all(|d| d.abs() < 0.5), "Abstand: kein Einzug: {x:?}");
    // Typst default of the line spacing 0.65 em; the paragraph spacing replaces it between paragraphs.
    assert!(
        ((par_gap - line_gap) - (1.2 - 0.65) * 10.0).abs() < 0.3,
        "Abstand 1,2 em: Absätze {:.2} pt weiter als Zeilen, erwartet {:.2}",
        par_gap - line_gap,
        (1.2 - 0.65) * 10.0
    );
}

/// Right edges of the lines in the type area (without the last line of each paragraph).
fn right_edges(justify: bool) -> Vec<f64> {
    let s = TypesetSettings { justify: Some(justify), hyphenate: Some(false), ..a5_margins() };
    let main = book("", &s, &paragraph(0, 8));
    let (_, pages) = set(&main);
    let rows = lines(&pages[0]);
    // Without the last line of the paragraph and without lines that end with a punctuation mark: Typst lets
    // punctuation marks hang into the margin (optical margin alignment, measured ≈ 1.9 pt).
    rows[..rows.len() - 1]
        .iter()
        .filter(|l| line_text(l).trim_end().chars().last().is_some_and(char::is_alphabetic))
        .map(|l| l.iter().map(|t| t.x_pt + t.width_pt).fold(f64::MIN, f64::max))
        .collect()
}

#[test]
fn justify_aligns_the_right_edges() {
    let justified = right_edges(true);
    let spread = |e: &[f64]| e.iter().cloned().fold(f64::MIN, f64::max) - e.iter().cloned().fold(f64::MAX, f64::min);
    assert!(justified.len() >= 5, "genug Zeilen: {}", justified.len());
    assert!(spread(&justified) < 0.6, "Blocksatz: alle rechten Kanten gleich: {justified:?}");
    let ragged = right_edges(false);
    assert!(spread(&ragged) > 5.0, "Flattersatz: rechte Kanten unterschiedlich: {ragged:?}");
}

#[test]
fn hyphenation_follows_the_setting_and_the_language() {
    // A freely invented long word in a narrow column.
    let word = "Leuchtturmwärterinnenversammlungsprotokollbuch";
    let text = format!("{} {word} {word} {word} {}", paragraph(0, 1), paragraph(1, 1));
    let hyphens = |hyphenate: bool, lang: &str| {
        let s = TypesetSettings {
            hyphenate: Some(hyphenate),
            lang: Some(lang.into()),
            justify: Some(true),
            page_format: Some(PageFormat::Custom { width_mm: 80.0, height_mm: 160.0 }),
            margins: Some(Margins {
                inside_mm: 10.0,
                outside_mm: 10.0,
                top_mm: 10.0,
                bottom_mm: 10.0,
                binding_mm: 0.0,
            }),
            ..Default::default()
        };
        let (_, pages) = set(&book("", &s, &text));
        // Typst sets the hyphen as a piece of its own with the text U+00AD (soft hyphen).
        pages
            .iter()
            .flat_map(|p| lines(p))
            .filter(|l| line_text(l).trim_end().ends_with(['-', '\u{ad}', '\u{2010}']))
            .count()
    };
    assert!(hyphens(true, "de") > 0, "Silbentrennung (de): mindestens eine Zeile endet mit Trennstrich");
    assert_eq!(hyphens(false, "de"), 0, "ohne Silbentrennung kein Trennstrich am Zeilenende");
    assert!(hyphens(true, "de-CH") > 0, "Sprache mit Region (de-CH) trennt ebenfalls");
}

// ---- Page numbers, running heads, footnotes ------------------------------------------------------

/// Book with front matter (three pages without header and footer, like `cover::frame_parts`), part and three
/// chapters.
fn titled_book(s: &TypesetSettings) -> String {
    let titelei = "#page(header: none, footer: none, numbering: none, { align(center, text(size: 16pt)[Gartenbuch]) })\n\
                   #page(header: none, footer: none, numbering: none, { hide[ ] })\n\
                   #page(header: none, footer: none, numbering: none, { [Impressum] })\n";
    let body = format!(
        "{titelei}{}\n{}{}{}",
        part_line("Erster Teil"),
        chapter("Frühling <k1>", 8),
        chapter("Sommer <k2>", 8),
        chapter("Herbst <k3>", 4)
    );
    book("#set document(title: \"Gartenbuch\")", s, &body)
}

/// Page number in the footer (below the type area) or header (above it): (text, x) of the digit pieces.
fn folios(page: &[PlacedText], top: f64, bottom: f64) -> Vec<(String, f64, bool)> {
    page.iter()
        .filter(|t| {
            (t.y_pt < top || t.y_pt > bottom) && t.text.chars().all(|c| c.is_ascii_digit()) && !t.text.is_empty()
        })
        .map(|t| (t.text.clone(), t.x_pt + t.width_pt / 2.0, t.y_pt < top))
        .collect()
}

fn numbered(position: PageNumberPosition, start: PageNumberStart, head: Option<RunningHead>) -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::RightPage),
        page_numbers: Some(PageNumbers { position, start }),
        running_head: head,
        ..a5_margins()
    }
}

#[test]
fn page_numbers_sit_outside_and_start_at_the_first_chapter() {
    let s = numbered(PageNumberPosition::BottomOutside, PageNumberStart::FirstChapter, None);
    let main = titled_book(&s);
    let (rendered, pages) = set(&main);
    let (w, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    let first = heading_page(&rendered, &main, "= Frühling");
    for (i, page) in pages.iter().enumerate() {
        let n = i + 1;
        let f = folios(page, top, bottom);
        if n < first {
            assert!(f.is_empty(), "vor dem ersten Kapitel keine Zahl (Seite {n}): {f:?}");
            continue;
        }
        if blank(page, top, bottom) {
            assert!(f.is_empty(), "Leerseite {n} ohne Zahl: {f:?}");
            continue;
        }
        assert_eq!(f.len(), 1, "genau eine Seitenzahl auf Seite {n}: {f:?}");
        let (text, center, in_head) = &f[0];
        assert!(!in_head, "unten: Seite {n}");
        assert_eq!(text, &(n - first + 1).to_string(), "Seite {n}: Zählung beginnt beim ersten Kapitel mit 1");
        // Outside: odd (right) page at the right of the type area, even at the left.
        if n % 2 == 1 {
            assert!(*center > w - mm(15.0) - 15.0, "Seite {n}: Zahl rechts außen ({center:.1} pt)");
        } else {
            assert!(*center < mm(15.0) + 15.0, "Seite {n}: Zahl links außen ({center:.1} pt)");
        }
    }
}

#[test]
fn page_numbers_bottom_center_top_outside_and_hidden() {
    let (w, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    let middle = w / 2.0 + (mm(25.0) - mm(15.0)) / 2.0; // centre of the type area on odd pages
    let s = numbered(PageNumberPosition::BottomCenter, PageNumberStart::FirstPage, None);
    let (_, pages) = set(&titled_book(&s));
    let n = pages.len(); // the last page carries text
    let f = folios(&pages[n - 1], top, bottom);
    assert_eq!(f.len(), 1, "unten mittig: eine Zahl auf der letzten Seite: {f:?}");
    let mid = if n % 2 == 1 { middle } else { w / 2.0 - (mm(25.0) - mm(15.0)) / 2.0 };
    assert!((f[0].1 - mid).abs() < 1.0, "unten mittig: {:.1} pt, Mitte des Satzspiegels {mid:.1} pt", f[0].1);
    assert_eq!(f[0].0, n.to_string(), "ab der ersten Seite: physische Seitenzahl (Titelei zählt mit)");

    let s = numbered(PageNumberPosition::TopOutside, PageNumberStart::FirstPage, None);
    let (_, pages) = set(&titled_book(&s));
    let last = pages.len();
    let f = folios(&pages[last - 1], top, bottom);
    assert!(f.len() == 1 && f[0].2, "oben außen: die Zahl steht über dem Satzspiegel: {f:?}");

    let s = numbered(PageNumberPosition::Hidden, PageNumberStart::FirstPage, None);
    let (_, pages) = set(&titled_book(&s));
    assert!(pages.iter().all(|p| folios(p, top, bottom).is_empty()), "keine: auf keiner Seite eine Zahl");
}

#[test]
fn reference_with_page_shows_the_printed_number_after_a_front_matter() {
    let s = numbered(PageNumberPosition::BottomOutside, PageNumberStart::FirstChapter, None);
    // Reference in the first chapter to the second, in the form of xref_snippet (Round 30a).
    let snippet = xref::xref_snippet(2, "den Sommer", true);
    let main = titled_book(&s).replace("= Frühling <k1>\n\n", &format!("= Frühling <k1>\n\nSiehe {snippet}.\n\n"));
    let (rendered, pages) = set(&main);
    let (_, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    let target = heading_page(&rendered, &main, "= Sommer");
    let printed = folios(&pages[target - 1], top, bottom);
    assert_eq!(printed.len(), 1, "die Zielseite trägt eine Zahl: {printed:?}");
    let reference = pages
        .iter()
        .flat_map(|p| lines(p))
        .map(|l| line_text(&l))
        .find(|t| t.contains("S."))
        .expect("Verweis mit Seitenzahl gesetzt");
    assert!(
        reference.contains(&format!("S.\u{a0}{}", printed[0].0)) || reference.contains(&format!("S. {}", printed[0].0)),
        "„S. n“ ({reference}) entspricht der gedruckten Zahl {} der Zielseite {target}",
        printed[0].0
    );
    assert!(target > 5, "die Titelei und der Teil liegen davor (Zielseite {target}), die Zahl ist also nicht physisch");
}

#[test]
fn running_head_shows_chapter_or_title_but_not_on_opening_part_and_blank_pages() {
    let (_, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    for (head, want) in [(RunningHead::Chapter, None), (RunningHead::WorkTitle, Some("Gartenbuch"))] {
        let s = numbered(PageNumberPosition::BottomOutside, PageNumberStart::FirstChapter, Some(head));
        let main = titled_book(&s);
        let (rendered, pages) = set(&main);
        let openings: Vec<usize> =
            ["= Frühling", "= Sommer", "= Herbst"].iter().map(|c| heading_page(&rendered, &main, c)).collect();
        let part = heading_page(&rendered, &main, "#heading(level: 1, numbering: none)[Erster Teil]");
        let mut shown = 0;
        for (i, page) in pages.iter().enumerate() {
            let n = i + 1;
            let heads: Vec<&str> = page.iter().filter(|t| t.y_pt < top).map(|t| t.text.as_str()).collect();
            if n <= 3 || n == part || openings.contains(&n) || blank(page, top, bottom) {
                assert!(
                    heads.is_empty(),
                    "{head:?}: kein Kolumnentitel auf Seite {n} (Titelei/Teil/Kapitelanfang/leer): {heads:?}"
                );
                continue;
            }
            let expected = want.map(str::to_owned).unwrap_or_else(|| {
                let chapter = openings.iter().rposition(|&o| o < n).expect("Seite hinter einem Kapitelanfang");
                ["Frühling", "Sommer", "Herbst"][chapter].to_owned()
            });
            assert_eq!(heads.join(" "), expected, "{head:?}: Kolumnentitel auf Seite {n}");
            shown += 1;
        }
        assert!(shown >= 3, "{head:?}: auf mehreren Seiten gezeigt ({shown})");
    }
    let s = numbered(PageNumberPosition::BottomOutside, PageNumberStart::FirstChapter, Some(RunningHead::Off));
    let (_, pages) = set(&titled_book(&s));
    assert!(pages.iter().all(|p| p.iter().all(|t| t.y_pt >= top)), "aus: kein Text über dem Satzspiegel");
    let _ = bottom;
}

/// Finding R29: two parts with one chapter each; before the second part an empty page arises in the middle of
/// the book (the part starts on the right). Without the setting „Teil-Seite“, but with page numbers and running
/// head, it used to carry "Alpha" and the number, because the part rule from Round 14 set no mark
/// `<baruch-satz-end>`. Now it is entirely empty – with and without chapter rule, without and with `part_page`.
#[test]
fn the_blank_page_before_a_second_part_stays_empty() {
    let (_, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    let base = TypesetSettings {
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstPage,
        }),
        running_head: Some(RunningHead::Chapter),
        ..a5_margins()
    };
    let cases = [
        ("ohne Kapitelregel, ohne Teil-Seite", None, None),
        ("rechte Seite, ohne Teil-Seite", Some(ChapterStart::RightPage), None),
        ("ohne Kapitelregel, Teil rechts", None, Some(PartPage { side: PartSide::Right, blank_back: false })),
    ];
    for (name, chapter_start, part_page) in cases {
        let s = TypesetSettings { chapter_start, part_page, ..base.clone() };
        // Depending on the length of Alpha it ends on an even or odd page; only in the second case an empty page
        // arises before the part starting on the right. Several lengths, so that it certainly occurs.
        let (mut blank_seen, mut text_seen) = (0, 0);
        for paragraphs in [2, 4, 6, 8, 10] {
            let body = format!(
                "{}\n{}{}\n{}",
                part_line("Eins"),
                chapter("Alpha", paragraphs),
                part_line("Zwei"),
                chapter("Beta", 2)
            );
            let main = book("", &s, &body);
            let (rendered, pages) = set(&main);
            let second = heading_page(&rendered, &main, "#heading(level: 1, numbering: none)[Zwei]");
            let alpha = heading_page(&rendered, &main, "= Alpha");
            assert!(second % 2 == 1, "{name}/{paragraphs}: der zweite Teil steht rechts (Seite {second})");
            for n in (alpha + 1)..second {
                let page = &pages[n - 1];
                if blank(page, top, bottom) {
                    assert!(
                        page.is_empty(),
                        "{name}/{paragraphs}: Leerseite {n} ohne Zahl und Kolumnentitel: {page:?}"
                    );
                    blank_seen += 1;
                } else {
                    // Cross-check: text pages carry their number (the mark does not silence too many pages).
                    assert_eq!(
                        folios(page, top, bottom).len(),
                        1,
                        "{name}/{paragraphs}: Textseite {n} trägt ihre Zahl"
                    );
                    text_seen += 1;
                }
            }
        }
        assert!(blank_seen > 0, "{name}: mindestens eine Länge ergibt eine Leerseite vor Teil Zwei");
        assert!(text_seen > 0, "{name}: mindestens eine Folgeseite von Alpha mit Text geprüft");
    }
}

#[test]
fn part_page_rule_for_marks_only_when_a_rule_block_exists() {
    let labels = vec!["I".to_owned()];
    assert_eq!(
        part_page_rule_for(&labels, &TypesetSettings::default()),
        baruch_typst::part_page_rule(&labels),
        "ohne Satz-Einstellungen byte-gleich zur Regel aus Runde 14"
    );
    let numbers_only = TypesetSettings {
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstPage,
        }),
        ..Default::default()
    };
    assert!(
        part_page_rule_for(&labels, &numbers_only).contains("<baruch-satz-end>"),
        "mit Regelblock trägt die Teil-Regel die Marke vor dem Umbruch (Leerseite davor erkennen)"
    );
    let reference_only = TypesetSettings { xref_with_page: Some(true), ..Default::default() };
    assert_eq!(
        part_page_rule_for(&labels, &reference_only),
        baruch_typst::part_page_rule(&labels),
        "nur der Verweis-Schalter ergibt keinen Regelblock und damit keine Änderung"
    );
}

/// Integrator R29: without a chapter rule the mark `<baruch-satz-end>` is at the same place as its heading.
/// Before, `baruch-satz-quiet` therefore took all pages up to the next chapter heading for empty – without page
/// number and running head. Now every page with text after the first chapter carries its number, and every one
/// except the chapter starts and the part page the running head.
#[test]
fn without_a_chapter_rule_every_text_page_keeps_number_and_head() {
    let s = TypesetSettings {
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstChapter,
        }),
        running_head: Some(RunningHead::Chapter),
        ..a5_margins()
    };
    let main = titled_book(&s);
    let (rendered, pages) = set(&main);
    let (_, h) = a5();
    let (top, bottom) = (mm(18.0), h - mm(22.0));
    let openings: Vec<usize> =
        ["= Frühling", "= Sommer", "= Herbst"].iter().map(|c| heading_page(&rendered, &main, c)).collect();
    let part = heading_page(&rendered, &main, "#heading(level: 1, numbering: none)[Erster Teil]");
    let first = openings[0];
    let mut checked = 0;
    for (i, page) in pages.iter().enumerate().skip(first - 1) {
        let n = i + 1;
        if n == part {
            continue;
        }
        if blank(page, top, bottom) {
            assert!(page.is_empty(), "Leerseite {n} ohne Zahl und Kolumnentitel: {page:?}");
            continue;
        }
        let f = folios(page, top, bottom);
        assert_eq!(f.len(), 1, "Seite {n} trägt ihre Zahl (ohne Kapitelregel): {f:?}");
        assert_eq!(f[0].0, (n - first + 1).to_string(), "Seite {n}: Zählung ab dem ersten Kapitel");
        let heads: Vec<&str> = page.iter().filter(|t| t.y_pt < top).map(|t| t.text.as_str()).collect();
        if openings.contains(&n) {
            assert!(heads.is_empty(), "Kapitelanfang {n} ohne Kolumnentitel: {heads:?}");
        } else {
            let chapter = openings.iter().rposition(|&o| o < n).unwrap();
            assert_eq!(heads.join(" "), ["Frühling", "Sommer", "Herbst"][chapter], "Kolumnentitel auf Seite {n}");
            checked += 1;
        }
    }
    assert!(checked >= 3, "mehrere Folgeseiten geprüft ({checked})");
}

#[test]
fn footnote_marks_and_restart_per_chapter() {
    let body = format!(
        "= Eins\n\n{}#footnote[Erste erfundene Anmerkung.] {}#footnote[Zweite Anmerkung.]\n\n= Zwei\n\n{}#footnote[Dritte Anmerkung.]\n",
        paragraph(0, 2),
        paragraph(1, 1),
        paragraph(2, 2)
    );
    let marks = |numbering: FootnoteNumbering, restart: FootnoteRestart| {
        let s = TypesetSettings {
            footnotes: Some(Footnotes { numbering, restart }),
            chapter_start: Some(ChapterStart::NewPage),
            ..a5_margins()
        };
        let (_, pages) = set(&book("", &s, &body));
        // The lines are one text piece each; a footnote mark (superscript, font features of its own) is a piece of
        // its own of one character – in the text and at the foot.
        pages
            .iter()
            .map(|p| {
                p.iter()
                    .filter(|t| t.text.chars().count() == 1 && !t.text.trim().is_empty())
                    .map(|t| t.text.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let arabic = marks(FootnoteNumbering::Arabic, FootnoteRestart::Continuous);
    assert_eq!(arabic[0], ["1", "2", "1", "2"], "Ziffern im Text und am Fuß von Seite 1");
    assert_eq!(arabic[1], ["3", "3"], "fortlaufend: die Anmerkung im zweiten Kapitel trägt 3");
    let restarted = marks(FootnoteNumbering::Arabic, FootnoteRestart::PerChapter);
    assert_eq!(restarted[1], ["1", "1"], "je Kapitel neu: das zweite Kapitel beginnt wieder mit 1");
    let symbols = marks(FootnoteNumbering::Symbols, FootnoteRestart::PerChapter);
    assert_eq!(symbols[0], ["*", "†", "*", "†"], "Symbole *, †");
    let letters = marks(FootnoteNumbering::Letters, FootnoteRestart::PerChapter);
    assert_eq!(letters[0], ["a", "b", "a", "b"], "Buchstaben a, b");
}

#[test]
fn widows_and_orphans_are_avoided_only_when_set() {
    // A paragraph of five lines after growing whitespace: at some point it breaks. Without protection, at some
    // point exactly one line stays alone on a page; with protection never.
    let lone_line = |avoid: bool| {
        let mut found = false;
        for space in (0..60).map(|i| 300.0 + f64::from(i) * 4.0) {
            let s = TypesetSettings {
                widows_orphans: Some(avoid),
                page_format: Some(PageFormat::Custom { width_mm: 100.0, height_mm: 160.0 }),
                margins: Some(Margins {
                    inside_mm: 10.0,
                    outside_mm: 10.0,
                    top_mm: 10.0,
                    bottom_mm: 10.0,
                    binding_mm: 0.0,
                }),
                justify: Some(false),
                ..Default::default()
            };
            let main = book("", &s, &format!("#v({space}pt)\n\n{}\n", paragraph(0, 4)));
            let (_, pages) = set(&main);
            if pages.len() == 2 {
                let counts: Vec<usize> = pages.iter().map(|p| lines(p).len()).collect();
                if counts.contains(&1) {
                    found = true;
                }
            }
        }
        found
    };
    assert!(lone_line(false), "ohne Schutz (0 %) entsteht bei einem Umbruch eine einzelne Zeile");
    assert!(!lone_line(true), "mit Schutz bleibt nie eine einzelne Zeile allein (Schusterjunge/Hurenkind)");
}

#[test]
fn page_texts_reports_baseline_font_and_size() {
    let main = "#set page(width: 100mm, height: 50mm, margin: 10mm)\n#set text(font: \"Libertinus Serif\", size: 12pt)\nGarten\n";
    let pages = page_texts(&source(main)).expect("setzt ohne Fehler");
    assert_eq!(pages.len(), 1, "eine Seite");
    let t = &pages[0][0];
    assert_eq!(t.text, "Garten", "Text des Stücks");
    assert!((t.x_pt - mm(10.0)).abs() < 0.01, "x am linken Rand: {}", t.x_pt);
    assert!(t.y_pt > mm(10.0) && t.y_pt < mm(10.0) + 12.0, "Grundlinie unter dem oberen Rand: {}", t.y_pt);
    assert!((t.size_pt - 12.0).abs() < 0.01 && t.width_pt > 20.0, "Größe und Breite: {t:?}");
    assert_eq!(t.font, "Libertinus Serif", "Familienname");
    assert_eq!(page_texts(&source("#let x = (")), None, "Satzfehler → None");
}

#[test]
fn preview_source_sets_at_least_six_pages_with_working_rules() {
    let s = TypesetSettings::from_json(
        r#"{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},"page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter","footnotes":{"numbering":"symbols","restart":"per_chapter"}}"#,
    )
    .expect("Einstellungen lesbar");
    let main = preview_source(&s, "#set document(title: \"Probe\")", "Erfundenes Buch");
    let (rendered, pages) = set(&main);
    assert!(pages.len() >= 6, "mindestens sechs Seiten Mustertext: {}", pages.len());
    let (w, h) = a5();
    assert!(
        page_sizes_pt(&rendered).iter().all(|(pw, ph)| (pw - w).abs() < 0.5 && (ph - h).abs() < 0.5),
        "A5 wirkt auf allen Seiten"
    );
    assert!(pages[1].is_empty(), "leere Rückseite der Teil-Seite");
    let chapters: Vec<usize> = rendered
        .headings
        .iter()
        .filter(|hp| main.lines().nth(hp.line - 1).is_some_and(|l| l.starts_with("= ")))
        .map(|hp| hp.page)
        .collect();
    assert_eq!(chapters.len(), 3, "drei Kapitel gesetzt");
    assert!(chapters.iter().all(|p| p % 2 == 1), "Kapitel auf rechten Seiten: {chapters:?}");
    assert_eq!(chapters[0], 3, "erstes Kapitel nach Teil-Seite und leerer Rückseite");
    let foot: Vec<&str> = pages[2]
        .iter()
        .filter(|t| t.y_pt > h - mm(20.0) && t.text.chars().all(|c| c.is_ascii_digit()))
        .map(|t| t.text.as_str())
        .collect();
    assert_eq!(foot, ["1"], "Zählung ab dem ersten Kapitel: die erste Kapitelseite trägt 1");
    let marks = pages.iter().flatten().filter(|t| t.text == "*").count();
    assert_eq!(marks, 6, "je Kapitel eine Fußnote mit „*“ (Neubeginn je Kapitel), im Text und am Fuß");
}
