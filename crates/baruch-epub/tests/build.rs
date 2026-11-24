// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 20a (epub): Dateien des EPUB aus der Eingabe – Aufteilung, Fußnoten, Querverweise, Bilder, Inseln,
//! Werkvorlage, Metadaten. Testwerk frei erfunden (`baruch_epub::sample`), Abwandlungen hier im Test.

use baruch_epub::sample::{self, KEYS};
use baruch_epub::{EpubBook, EpubError, EpubInput, EpubSection, FootnoteMode, ItemKind, build};

fn text(book: &EpubBook, href: &str) -> String {
    let item = book.item(href).unwrap_or_else(|| panic!("{href} fehlt im Buch: {:?}", hrefs(book)));
    String::from_utf8(item.bytes.clone()).expect("UTF-8")
}

fn hrefs(book: &EpubBook) -> Vec<&str> {
    book.items.iter().map(|i| i.href.as_str()).collect()
}

fn codes(book: &EpubBook) -> Vec<&str> {
    book.warnings.iter().map(|w| w.code.as_str()).collect()
}

fn section(key: i64, level: u8, title: &str, body: &str) -> EpubSection {
    EpubSection { key, level, number: None, title: title.into(), body: body.into() }
}

#[test]
fn every_part_and_chapter_is_its_own_file_in_spine_order() {
    let book = build(&sample::input()).unwrap();
    let [part, ch1, s11, ch2, s21] = KEYS;
    assert_eq!(
        book.spine,
        vec!["cover", "title", "imprint", "dedication", &format!("k{part}"), &format!("k{ch1}"), &format!("k{ch2}")],
        "Umschlag und Titelei vorn, dann Teil und Kapitel in Gliederungsreihenfolge (Vertrag §0.2, §0.3)"
    );
    assert!(book.item(&format!("text/k{s11}.xhtml")).is_none(), "Ebene 2 bekommt keine eigene Datei");
    assert!(book.item(&format!("text/k{s21}.xhtml")).is_none(), "Ebene 2 bekommt keine eigene Datei");
    let chapter = text(&book, &format!("text/k{ch1}.xhtml"));
    assert!(chapter.contains(&format!("<h2 id=\"k{s11}\">")), "1.1 steht in der Datei von Kapitel 1: {chapter}");
    assert!(
        text(&book, &format!("text/k{ch2}.xhtml")).contains(&format!("id=\"k{s21}\"")),
        "2.1 steht in der Datei von Kapitel 2"
    );
    let part_file = text(&book, &format!("text/k{part}.xhtml"));
    assert!(
        part_file.contains("<section class=\"baruch-part\" epub:type=\"part\">"),
        "Teil-Seite als section.baruch-part mit epub:type: {part_file}"
    );
    assert!(
        chapter.contains("<section class=\"baruch-chapter\" epub:type=\"chapter\">"),
        "Kapitel als section.baruch-chapter mit epub:type"
    );
    assert_eq!(book.item(&format!("text/k{part}.xhtml")).unwrap().kind, ItemKind::Part, "Art Teil");
    assert_eq!(book.item(&format!("text/k{ch1}.xhtml")).unwrap().key, Some(ch1), "Schlüssel des Kapitels");
    let css = text(&book, "style/book.css");
    assert!(
        css.contains("section.baruch-chapter { break-before: page; page-break-before: always; }"),
        "Umbruch vor Kapitel-Dateien auch ohne Satz-Einstellung: {css}"
    );
    assert!(css.contains("section.baruch-part { break-before: page;"), "Umbruch vor Teil-Dateien: {css}");
    assert!(chapter.contains("<span class=\"baruch-number\">1</span> Der Platz"), "Nummer aus der Gliederung");
}

#[test]
fn sections_before_the_first_chapter_go_to_front_xhtml() {
    let mut input = sample::input();
    input.sections.insert(0, section(5, 2, "Vorweg", "== Vorweg <k5>\n\nEin Wort vorab.\n"));
    let book = build(&input).unwrap();
    let front = text(&book, "text/front.xhtml");
    assert!(front.contains("Ein Wort vorab."), "Text vor dem ersten Teil steht in front.xhtml: {front}");
    let at = book.spine.iter().position(|id| id == "front").expect("front im Spine");
    assert_eq!(book.spine[at + 1], format!("k{}", KEYS[0]), "front.xhtml vor dem ersten Teil");
}

#[test]
fn chapter_files_stay_even_with_continuous_chapters() {
    let mut input = sample::input();
    input.typeset.chapter_start = Some(baruch_typst::typeset::ChapterStart::Continuous);
    let book = build(&input).unwrap();
    assert!(book.item(&format!("text/k{}.xhtml", KEYS[3])).is_some(), "Kapitel 2 bleibt eine eigene Datei");
    let css = text(&book, "style/book.css");
    assert!(
        css.contains("section.baruch-chapter { break-before: auto;"),
        "fortlaufend ohne erzwungenen Umbruch: {css}"
    );
}

#[test]
fn popup_footnotes_point_to_an_aside_in_the_same_file() {
    let book = build(&sample::input()).unwrap();
    let chapter = text(&book, &format!("text/k{}.xhtml", KEYS[1]));
    assert!(
        chapter.contains("<a class=\"baruch-noteref\" id=\"fnref1\" href=\"#fn1\" epub:type=\"noteref\""),
        "Verweis auf die Fußnote in derselben Datei: {chapter}"
    );
    assert!(
        chapter.contains("<aside id=\"fn1\" epub:type=\"footnote\""),
        "Fußnote als aside epub:type=footnote: {chapter}"
    );
    assert!(chapter.contains("href=\"#fnref1\""), "Rückverweis zum Text");
    assert!(chapter.contains("<ol class=\"baruch-footnotes\">"), "Liste ol.baruch-footnotes, lesbar ohne Popup");
    let aside = chapter.find("<aside").unwrap();
    assert!(chapter.find("</section>").unwrap() > aside, "Fußnoten am Ende der Kapiteldatei, in ihrer section");
    assert!(chapter[aside..].contains("vor zwölf Jahren"), "Text der Fußnote steht im aside");
    assert!(book.item("text/notes.xhtml").is_none(), "im Popup-Modus keine Endnoten-Datei");
}

#[test]
fn endnotes_collect_every_note_in_notes_xhtml_with_backlinks() {
    let mut input = sample::input();
    input.footnotes = FootnoteMode::Endnotes;
    let book = build(&input).unwrap();
    let ch1 = KEYS[1];
    let chapter = text(&book, &format!("text/k{ch1}.xhtml"));
    assert!(chapter.contains("href=\"notes.xhtml#fn1\""), "Verweis in die Endnoten: {chapter}");
    assert!(!chapter.contains("<aside"), "keine Fußnote mehr in der Kapiteldatei");
    let notes = text(&book, "text/notes.xhtml");
    assert!(notes.contains("<li id=\"fn1\" epub:type=\"endnote\">"), "Endnote mit Ziel-id: {notes}");
    assert!(notes.contains(&format!("href=\"k{ch1}.xhtml#fnref1\"")), "Rückverweis in die Kapiteldatei: {notes}");
    assert_eq!(book.spine.last().map(String::as_str), Some("notes"), "Endnoten am Ende des Spine");
    assert!(book.toc.iter().any(|t| t.href == "text/notes.xhtml"), "Anmerkungen im Inhaltsverzeichnis");
}

#[test]
fn endnotes_stand_on_the_top_level_of_the_toc_even_in_works_with_parts() {
    // Das Testwerk hat einen Teil (Ebene 0); mit Ebene 1 hinge „Anmerkungen“ als Kapitel unter dem letzten Teil.
    let mut input = sample::input();
    input.footnotes = FootnoteMode::Endnotes;
    let book = build(&input).unwrap();
    let notes = book.toc.iter().find(|t| t.href == "text/notes.xhtml").expect("Anmerkungen im Inhaltsverzeichnis");
    let part = book.toc.iter().find(|t| t.key == Some(KEYS[0])).expect("Teil im Inhaltsverzeichnis");
    assert_eq!(notes.level, part.level, "Anmerkungen auf der Ebene des Teils, nicht darunter");
    let nav = text(&book, "nav.xhtml");
    let at = nav.find("text/notes.xhtml").expect("Anmerkungen in nav.xhtml");
    let before = &nav[..at];
    assert_eq!(
        before.matches("<ol").count() - before.matches("</ol>").count(),
        1,
        "in nav.xhtml nur in der äußersten Liste geöffnet, nicht in der des Teils: {nav}"
    );
    let ncx = text(&book, "toc.ncx");
    let at = ncx.find("text/notes.xhtml").expect("Anmerkungen in toc.ncx");
    let before = &ncx[..at];
    assert_eq!(
        before.matches("<navPoint").count() - before.matches("</navPoint>").count(),
        1,
        "in toc.ncx nur im eigenen navPoint, nicht in dem des Teils: {ncx}"
    );
}

#[test]
fn a_label_inside_a_footnote_points_to_the_file_where_the_note_stands() {
    let ch2 = KEYS[3];
    let mut input = sample::input();
    input.sections[3].body.push_str("\nEine Bemerkung.#footnote[Notiz mit Marke <notiz>.]\n\nSiehe @notiz.\n");
    // Popup: die Fußnote steht in der Kapiteldatei, der Verweis bleibt dort.
    let popup = build(&input).unwrap();
    let chapter = text(&popup, &format!("text/k{ch2}.xhtml"));
    assert!(chapter.contains("id=\"lbl-notiz\""), "Sprungmarke in der Kapiteldatei: {chapter}");
    assert!(chapter.contains(&format!("href=\"k{ch2}.xhtml#lbl-notiz\"")), "Verweis in dieselbe Datei: {chapter}");
    // Endnoten: die Sprungmarke steht in notes.xhtml, also muss der Verweis dorthin zeigen (sonst toter Link).
    input.footnotes = FootnoteMode::Endnotes;
    let book = build(&input).unwrap();
    let notes = text(&book, "text/notes.xhtml");
    assert!(notes.contains("id=\"lbl-notiz\""), "Sprungmarke steht in notes.xhtml: {notes}");
    let chapter = text(&book, &format!("text/k{ch2}.xhtml"));
    assert!(chapter.contains("href=\"notes.xhtml#lbl-notiz\""), "Verweis zeigt auf notes.xhtml: {chapter}");
    assert!(!chapter.contains("id=\"lbl-notiz\""), "keine Sprungmarke in der Kapiteldatei");
}

#[test]
fn footnotes_restart_per_chapter_only_when_the_settings_say_so() {
    let mut input = sample::input();
    input.sections[3].body.push_str("\nNoch eine Bemerkung.#footnote[Zweite Fußnote.]\n");
    let book = build(&input).unwrap();
    let ch2 = text(&book, &format!("text/k{}.xhtml", KEYS[3]));
    assert!(ch2.contains("id=\"fn2\""), "ohne Einstellung fortlaufend wie Typst: {ch2}");
    assert!(ch2.contains("<ol class=\"baruch-footnotes\" start=\"2\">"), "die Liste zählt ab 2 weiter");
    input.typeset.footnotes = Some(baruch_typst::typeset::Footnotes {
        numbering: baruch_typst::typeset::FootnoteNumbering::Symbols,
        restart: baruch_typst::typeset::FootnoteRestart::PerChapter,
    });
    let book = build(&input).unwrap();
    let ch2 = text(&book, &format!("text/k{}.xhtml", KEYS[3]));
    assert!(ch2.contains("id=\"fn1\""), "je Kapitel neu ab 1: {ch2}");
    assert!(ch2.contains(">*</a></sup>"), "Zeichen * aus der Zählweise Symbole: {ch2}");
}

#[test]
fn a_cross_reference_links_to_the_chapter_file_of_its_target() {
    let book = build(&sample::input()).unwrap();
    let [_, ch1, s11, ch2, _] = KEYS;
    let chapter2 = text(&book, &format!("text/k{ch2}.xhtml"));
    assert!(
        chapter2.contains(&format!("<a class=\"baruch-xref\" href=\"k{ch1}.xhtml#k{s11}\">Boden und Licht</a>")),
        "Querverweis auf 1.1 zeigt in die Datei von Kapitel 1: {chapter2}"
    );
    assert!(text(&book, &format!("text/k{ch1}.xhtml")).contains(&format!("id=\"k{s11}\"")), "das Ziel-id gibt es");
}

#[test]
fn a_cross_reference_to_an_unknown_section_keeps_its_text_and_warns() {
    let mut input = sample::input();
    input.sections[2].body.push_str("\nSiehe #link(<k999>)[den Anhang].\n");
    let book = build(&input).unwrap();
    let chapter = text(&book, &format!("text/k{}.xhtml", KEYS[1]));
    assert!(chapter.contains("Siehe den Anhang."), "der Text bleibt: {chapter}");
    let w = book.warnings.iter().find(|w| w.code == "XREF_UNKNOWN").expect("Warnung XREF_UNKNOWN");
    assert_eq!(w.key, Some(KEYS[2]), "mit dem Schlüssel des Abschnitts");
}

#[test]
fn image_islands_become_img_with_alt_and_a_manifest_entry() {
    let book = build(&sample::input()).unwrap();
    let chapter = text(&book, &format!("text/k{}.xhtml", KEYS[1]));
    assert!(
        chapter.contains(
            "<figure class=\"baruch-figure\"><img class=\"baruch-image\" src=\"../images/bilder/weide.png\" \
             alt=\"Skizze der Weide am Bach\"/><figcaption>Die Weide</figcaption></figure>"
        ),
        "Abbildung mit Bild, Alternativtext und Unterschrift: {chapter}"
    );
    let weide = book.item("images/bilder/weide.png").expect("Bild im Manifest");
    assert_eq!(weide.media_type, "image/png", "Typ aus den Bildangaben");
    assert!(weide.id.starts_with("img-"), "id nach Namensregel img-n: {}", weide.id);
    let plan = text(&book, &format!("text/k{}.xhtml", KEYS[3]));
    assert!(plan.contains("src=\"../images/bilder/plan.png\" alt=\"\""), "Bild ohne Alternativtext: alt=\"\"");
    let warning = book.warnings.iter().find(|w| w.code == "IMAGE_NO_ALT").expect("Warnung IMAGE_NO_ALT");
    assert_eq!(warning.key, Some(KEYS[3]), "Warnung mit dem Schlüssel des Abschnitts");
    assert!(warning.message.contains("bilder/plan.png"), "Warnung nennt das Bild: {}", warning.message);
    assert!(!book.opf.contains("alternativeText"), "nicht jedes Bild hat alt → keine Zusage alternativeText");
}

#[test]
fn every_img_has_an_alt_attribute() {
    let book = build(&sample::input()).unwrap();
    for item in book.items.iter().filter(|i| i.media_type == "application/xhtml+xml") {
        let text = String::from_utf8(item.bytes.clone()).unwrap();
        for (at, _) in text.match_indices("<img ") {
            let tag = &text[at..at + text[at..].find("/>").unwrap()];
            assert!(tag.contains(" alt=\""), "{}: img ohne alt: {tag}", item.href);
        }
    }
}

#[test]
fn an_unknown_image_shows_a_note_and_warns() {
    let mut input = sample::input();
    input.sections[2].body.push_str("\n#image(\"bilder/fehlt.png\")\n");
    let book = build(&input).unwrap();
    let chapter = text(&book, &format!("text/k{}.xhtml", KEYS[1]));
    assert!(chapter.contains("Bild fehlt: bilder/fehlt.png"), "sichtbarer Hinweis statt still nichts: {chapter}");
    assert!(codes(&book).contains(&"IMAGE_MISSING"), "Warnung IMAGE_MISSING: {:?}", codes(&book));
}

#[test]
fn other_islands_are_omitted_with_a_warning_never_silently() {
    let mut input = sample::input();
    input.sections[2].body.push_str(
        "\n#set text(size: 12pt)\n\nDie Fläche ist $a b$ groß.\n\n// nur ein Kommentar\n\nDer Code `let x` bleibt.\n",
    );
    let book = build(&input).unwrap();
    let omitted: Vec<_> = book.warnings.iter().filter(|w| w.code == "ISLAND_OMITTED").collect();
    assert_eq!(omitted.len(), 2, "#set-Zeile und Formel ausgelassen, Kommentar nicht gemeldet: {omitted:?}");
    assert!(omitted.iter().all(|w| w.key == Some(KEYS[2])), "jede Warnung mit dem Schlüssel des Abschnitts");
    assert!(omitted[0].message.contains("#set text(size: 12pt)"), "Warnung nennt die Insel: {}", omitted[0].message);
    let chapter = text(&book, &format!("text/k{}.xhtml", KEYS[1]));
    assert!(!chapter.contains("#set"), "Typst-Code erscheint nicht im Text");
    assert!(chapter.contains("<p>Die Fläche ist  groß.</p>"), "Text um die Formel bleibt: {chapter}");
    assert!(chapter.contains("<code>let x</code>"), "Raw-Text wird code: {chapter}");
    assert!(!chapter.contains("Kommentar"), "Kommentare erscheinen nicht");
}

#[test]
fn a_level_one_pagebreak_rule_in_the_template_is_reported_as_mapped() {
    let mut input = sample::input();
    input.template.push_str("#show heading.where(level: 1): it => { pagebreak(weak: true, to: \"odd\"); it }\n");
    let book = build(&input).unwrap();
    let w = book.warnings.iter().find(|w| w.code == "TEMPLATE_PAGEBREAK_MAPPED").expect("Info MAPPED");
    assert!(w.message.contains("Zeile 3"), "Zeile der Regel genannt: {}", w.message);
    assert!(!codes(&book).contains(&"TEMPLATE_PAGEBREAK_UNMAPPED"), "keine Warnung für die Kapitelregel");
    let css = text(&book, "style/book.css");
    assert!(
        css.contains("section.baruch-chapter { break-before: right;"),
        "die erkannte Regel (to: odd) bestimmt den Umbruch, solange die Satz-Einstellung fehlt: {css}"
    );
}

#[test]
fn another_pagebreak_rule_in_the_template_is_reported_as_unmapped() {
    let mut input = sample::input();
    input.template.push_str("#show figure: it => {\n  pagebreak()\n  it\n}\n");
    let book = build(&input).unwrap();
    let w = book.warnings.iter().find(|w| w.code == "TEMPLATE_PAGEBREAK_UNMAPPED").expect("Warnung UNMAPPED");
    assert!(w.message.contains("#show figure"), "Regel genannt: {}", w.message);
    assert_eq!(w.key, None, "die Werkvorlage hat keinen Abschnittsschlüssel");
    assert!(!codes(&book).contains(&"TEMPLATE_PAGEBREAK_MAPPED"), "nichts abgebildet");
}

#[test]
fn metadata_land_in_the_opf_as_escaped_text() {
    let book = build(&sample::input()).unwrap();
    let opf = &book.opf;
    for (needle, why) in [
        ("<dc:identifier id=\"book-id\">urn:uuid:3f6c2a1e-8b4d-4c7a-9e21-5d0f7a9b1c42</dc:identifier>", "Kennung"),
        ("<dc:identifier id=\"isbn\">urn:isbn:9783161484100</dc:identifier>", "ISBN mit gültiger Prüfziffer"),
        ("<dc:title id=\"title\">Das Gartenhaus am Bach</dc:title>", "Titel"),
        ("<dc:title id=\"subtitle\">Ein Bautagebuch</dc:title>", "Untertitel"),
        ("<dc:creator id=\"creator\">Erika Beispiel</dc:creator>", "Autor"),
        ("<dc:language>de</dc:language>", "Sprache"),
        ("<dc:publisher>Beispielverlag</dc:publisher>", "Verlag"),
        ("<dc:date>2026</dc:date>", "Jahr"),
        ("<dc:rights>© 2026 Erika Beispiel</dc:rights>", "Rechte"),
        ("<dc:rights>Bildnachweis: Zeichnungen: Erika Beispiel</dc:rights>", "Bildnachweis"),
        ("<dc:description>Wie aus einer Weide am Bach ein Gartenhaus wurde.</dc:description>", "Beschreibung"),
        ("<meta property=\"dcterms:modified\">2026-11-09T12:00:00Z</meta>", "Stand"),
        ("<meta property=\"schema:accessMode\">textual</meta>", "Barrierefreiheit: Zugangsart"),
        ("<meta property=\"schema:accessibilityFeature\">tableOfContents</meta>", "Barrierefreiheit: Inhalt"),
        ("<meta property=\"schema:accessibilityHazard\">none</meta>", "Barrierefreiheit: keine Gefahren"),
        ("<meta property=\"schema:accessibilitySummary\">", "Barrierefreiheit: Zusammenfassung"),
    ] {
        assert!(opf.contains(needle), "{why} fehlt im OPF ({needle}): {opf}");
    }
}

#[test]
fn an_isbn_with_a_wrong_check_digit_stays_out_of_the_package() {
    let mut input = sample::input();
    input.meta.isbn = Some("9783161484101".into());
    let book = build(&input).unwrap();
    assert!(!book.opf.contains("urn:isbn"), "falsche Prüfziffer → keine ISBN: {}", book.opf);
    assert!(codes(&book).contains(&"ISBN_INVALID"), "ehrliche Warnung");
}

#[test]
fn book_details_with_markup_characters_stay_text() {
    let mut input = sample::input();
    input.meta.title = "Mühle <&> \"Rad\" #[strong]".into();
    input.meta.author = "A & B <script>".into();
    input.meta.description = "Klappe #emph[zu] <b>".into();
    input.imprint = vec!["© 2026 A & B, #set text(red)".into()];
    input.dedication = "Für <dich> & #mich".into();
    let book = build(&input).unwrap();
    assert!(
        book.opf.contains("<dc:title id=\"title\">Mühle &lt;&amp;&gt; &quot;Rad&quot; #[strong]</dc:title>"),
        "Titel maskiert, Typst-Zeichen als Text: {}",
        book.opf
    );
    assert!(book.opf.contains("A &amp; B &lt;script&gt;"), "Autor maskiert");
    assert!(book.opf.contains("Klappe #emph[zu] &lt;b&gt;"), "Beschreibung maskiert, nicht ausgewertet");
    let title = text(&book, "text/title.xhtml");
    assert!(title.contains("Mühle &lt;&amp;&gt; &quot;Rad&quot; #[strong]"), "Titelseite maskiert: {title}");
    assert!(!title.contains("<strong>"), "#[strong] wird nicht ausgewertet");
    assert!(text(&book, "text/imprint.xhtml").contains("<p>© 2026 A &amp; B, #set text(red)</p>"), "Impressum");
    assert!(text(&book, "text/dedication.xhtml").contains("Für &lt;dich&gt; &amp; #mich"), "Widmung");
    assert!(text(&book, "nav.xhtml").contains("<html"), "nav bleibt XHTML");
    assert!(text(&book, "toc.ncx").contains("<text>Mühle &lt;&amp;&gt; &quot;Rad&quot; #[strong]</text>"), "NCX");
}

#[test]
fn the_cover_is_the_first_file_and_marked_as_cover_image() {
    let book = build(&sample::input()).unwrap();
    let image = book.item("images/cover.png").expect("Umschlagbild");
    assert_eq!(image.id, "cover-image", "id nach Namensregel");
    assert_eq!(image.properties.as_deref(), Some("cover-image"), "EPUB 3: properties=cover-image");
    assert!(book.opf.contains("<meta name=\"cover\" content=\"cover-image\"/>"), "EPUB 2: meta name=cover");
    assert_eq!(book.spine[0], "cover", "cover.xhtml als erste Datei");
    let cover = text(&book, "text/cover.xhtml");
    assert!(
        cover.contains("src=\"../images/cover.png\" alt=\"Umschlag: Das Gartenhaus am Bach\""),
        "Umschlag mit Alternativtext: {cover}"
    );
    let mut input = sample::input();
    input.cover = None;
    let book = build(&input).unwrap();
    assert!(book.item("text/cover.xhtml").is_none() && !book.opf.contains("cover-image"), "ohne Umschlag nichts");
}

#[test]
fn front_matter_files_exist_only_when_switched_on() {
    let book = build(&sample::input()).unwrap();
    let imprint = text(&book, "text/imprint.xhtml");
    assert!(imprint.contains("<p>ISBN 978-3-16-148410-0</p>"), "Impressum aus imprint_lines: {imprint}");
    assert!(text(&book, "text/dedication.xhtml").contains("Für alle, die am Bach bauen."), "Widmung");
    let mut input = sample::input();
    input.front_matter = false;
    let book = build(&input).unwrap();
    for href in ["text/title.xhtml", "text/imprint.xhtml", "text/dedication.xhtml"] {
        assert!(book.item(href).is_none(), "{href} nur mit Titelei");
    }
}

#[test]
fn nav_has_a_nested_toc_and_landmarks() {
    let book = build(&sample::input()).unwrap();
    let nav = text(&book, "nav.xhtml");
    let [part, ch1, s11, ..] = KEYS;
    assert!(nav.contains("<nav epub:type=\"toc\" id=\"toc\""), "Inhaltsverzeichnis: {nav}");
    assert!(nav.contains("<nav epub:type=\"landmarks\""), "Landmarks");
    assert!(nav.contains("epub:type=\"cover\" href=\"text/cover.xhtml\""), "Landmark Umschlag");
    assert!(nav.contains(&format!("epub:type=\"bodymatter\" href=\"text/k{part}.xhtml\"")), "Landmark Text");
    let p = nav.find(&format!("text/k{part}.xhtml#k{part}")).unwrap();
    let c = nav.find(&format!("text/k{ch1}.xhtml#k{ch1}")).unwrap();
    let s = nav.find(&format!("text/k{ch1}.xhtml#k{s11}")).unwrap();
    assert!(p < c && c < s, "Reihenfolge Teil, Kapitel, Abschnitt");
    assert!(nav[p..c].contains("<ol>"), "Kapitel unter dem Teil verschachtelt");
    let ncx = text(&book, "toc.ncx");
    assert!(ncx.contains("<meta name=\"dtb:depth\" content=\"3\"/>"), "NCX-Tiefe Teil/Kapitel/Abschnitt: {ncx}");
    assert!(ncx.contains("playOrder=\"5\""), "fortlaufende playOrder");
}

#[test]
fn no_sections_and_bad_timestamps_are_refused() {
    let mut input = sample::input();
    input.sections.clear();
    assert_eq!(build(&input).unwrap_err(), EpubError::NoSections, "ohne Abschnitt kein EPUB");
    let mut input = sample::input();
    input.meta.modified = "gestern".into();
    assert!(matches!(build(&input), Err(EpubError::Invalid(_))), "dcterms:modified muss die feste Form haben");
}

#[test]
fn the_same_input_gives_the_same_book() {
    let a: EpubInput = sample::input();
    assert_eq!(build(&a).unwrap(), build(&a.clone()).unwrap(), "deterministisch");
}

#[test]
fn the_language_reaches_every_file() {
    let mut input = sample::input();
    input.meta.language = "de-CH".into();
    let book = build(&input).unwrap();
    for item in book.items.iter().filter(|i| i.media_type == "application/xhtml+xml") {
        let text = String::from_utf8(item.bytes.clone()).unwrap();
        assert!(text.contains("xml:lang=\"de-CH\" lang=\"de-CH\""), "{}: Sprache gesetzt", item.href);
    }
    assert!(book.opf.contains("<dc:language>de-CH</dc:language>"), "Sprache im Paket");
    input.meta.language = "kein tag".into();
    let book = build(&input).unwrap();
    assert!(book.opf.contains("<dc:language>de</dc:language>"), "ungültig → de");
    assert!(codes(&book).contains(&"LANGUAGE_INVALID"), "mit Warnung");
}

#[test]
fn without_a_language_the_settings_then_the_template_decide() {
    let mut input = sample::input();
    input.meta.language = String::new();
    input.template = "#set text(lang: \"en\")\n".into();
    let book = build(&input).unwrap();
    assert!(book.opf.contains("<dc:language>en</dc:language>"), "Sprache aus #set text(lang:) der Vorlage");
    input.typeset.lang = Some("fr".into());
    let book = build(&input).unwrap();
    assert!(book.opf.contains("<dc:language>fr</dc:language>"), "Satz-Einstellung vor der Vorlage");
    input.typeset.lang = None;
    input.template = String::new();
    let book = build(&input).unwrap();
    assert!(book.opf.contains("<dc:language>de</dc:language>"), "sonst Deutsch, nie leer");
}

/// R22 (§2.9): Mit Entwurf trägt die Titelseite den Hinweis (Text und Zusatz, maskiert) und dc:title/docTitle
/// „<Titel> (Entwurf)“; ohne Entwurf ist jede Datei byte-gleich zum Buch vorher.
#[test]
fn a_draft_marks_the_title_page_and_the_title_metadata_and_nothing_else_changes_without_it() {
    let plain = build(&sample::input()).unwrap();
    let mut input = sample::input();
    input.typeset.draft = None;
    let again = build(&input).unwrap();
    assert_eq!(plain, again, "draft: None ist dasselbe Buch wie vorher (byte-gleich)");

    input.typeset.draft = Some(baruch_typst::typeset::Draft {
        text: "Probe <1>".into(),
        note: "Stand 10.11.2026 & Co".into(),
        ..Default::default()
    });
    let draft = build(&input).unwrap();
    let title_page = text(&draft, "text/title.xhtml");
    assert!(
        title_page.contains("Entwurf: Probe &lt;1&gt; · Stand 10.11.2026 &amp; Co"),
        "Hinweis mit Text und Zusatz als maskierter Text auf der Titelseite: {title_page}"
    );
    let title = input.meta.title.trim().to_owned();
    assert!(
        draft.opf.contains(&format!("<dc:title id=\"title\">{title} (Entwurf)</dc:title>")),
        "dc:title trägt „(Entwurf)“: {}",
        draft.opf
    );
    assert!(
        text(&draft, "toc.ncx").contains(&format!("<docTitle><text>{title} (Entwurf)</text></docTitle>")),
        "auch der EPUB-2-Titel trägt „(Entwurf)“"
    );
    assert!(
        title_page.contains(&format!("<p class=\"baruch-title\">{title}</p>")),
        "die Titelseite zeigt den Titel selbst ohne Zusatz: {title_page}"
    );
    assert_eq!(
        text(&draft, "style/book.css"),
        text(&plain, "style/book.css"),
        "der Stil bleibt gleich (Hinweis mit Inline-Stil)"
    );

    // Ohne Titelei entsteht mit Entwurf trotzdem eine Titelseite (der Hinweis braucht einen Ort), aber kein Impressum.
    input.front_matter = false;
    let bare = build(&input).unwrap();
    assert!(bare.spine.iter().any(|id| id == "title"), "Titelseite im Spine: {:?}", bare.spine);
    assert!(!bare.spine.iter().any(|id| id == "imprint"), "Impressum nur mit Titelei: {:?}", bare.spine);
}
