// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a (Härtung, Reste aus R20): Fußnote in der Fußnote in richtiger Reihenfolge und Nummer, `@label` auf eine
//! Abbildung als „Abbildung n“, Inhalt eines komprimierten `mimetype`. Texte frei erfunden (`baruch_epub::sample`
//! und Abwandlungen hier).

use std::io::{Cursor, Read, Write};

use baruch_epub::check::{Severity, check_structure, codes};
use baruch_epub::sample::{self, KEYS};
use baruch_epub::xhtml::{Links, NoteTarget, Renderer, figure_label, footnote_list};
use baruch_epub::{EpubBook, FootnoteMode, build, package};
use baruch_typst::typeset::FootnoteNumbering;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

fn text(book: &EpubBook, href: &str) -> String {
    let item = book.item(href).unwrap_or_else(|| panic!("{href} fehlt im Buch"));
    String::from_utf8(item.bytes.clone()).expect("UTF-8")
}

/// Stelle von `needle` in `hay` (mit Meldung, wenn es fehlt).
fn at(hay: &str, needle: &str) -> usize {
    hay.find(needle).unwrap_or_else(|| panic!("„{needle}“ fehlt in: {hay}"))
}

#[test]
fn a_footnote_inside_a_footnote_comes_after_its_parent_with_the_next_number() {
    let links = Links::default();
    let mut r = Renderer::new(&links, "k5.xhtml", NoteTarget::SameFile, FootnoteNumbering::Arabic, 1);
    let mut out = String::new();
    let body = "= Der Steg\n\nDas Brett trägt.#footnote[So der Fährmann.#footnote[Er hieß Oswin.] Mehr nicht.] \
                Der Fluss steigt.#footnote[Im März.]\n";
    r.section(&mut out, 5, 1, None, "Der Steg", body);
    let numbers: Vec<usize> = r.notes.iter().map(|n| n.number).collect();
    assert_eq!(numbers, [1, 2, 3], "die Liste läuft in der Reihenfolge der Nummern: äußere, innere, nächste");
    assert!(r.notes[0].html.contains("So der Fährmann."), "Nummer 1 ist die äußere Fußnote: {:?}", r.notes[0]);
    assert!(
        r.notes[0].html.contains("href=\"#fn2\""),
        "die äußere verweist in ihrem Text auf die innere als Nummer 2: {:?}",
        r.notes[0]
    );
    assert_eq!(r.notes[1].html, "Er hieß Oswin.", "Nummer 2 ist die innere Fußnote");
    assert_eq!(r.notes[2].html, "Im März.", "Nummer 3 ist die nächste Fußnote des Absatzes");
    assert!(out.contains("id=\"fnref1\"") && out.contains("id=\"fnref3\""), "Verweise im Text tragen 1 und 3: {out}");

    let list = footnote_list(&r.notes);
    assert!(!list.contains(" start="), "die Liste beginnt bei 1, also ohne `start` (vorher verrutscht): {list}");
    assert!(
        at(&list, "id=\"fn1\"") < at(&list, "id=\"fn2\"") && at(&list, "id=\"fn2\"") < at(&list, "id=\"fn3\""),
        "die sichtbare Zählung der Liste (1, 2, 3) stimmt mit den Ankern überein: {list}"
    );
}

#[test]
fn nested_footnotes_keep_their_order_in_the_built_book_in_both_modes() {
    let ch2 = KEYS[3];
    for mode in [FootnoteMode::Popup, FootnoteMode::Endnotes] {
        let mut input = sample::input();
        input.footnotes = mode;
        // Alle vorhandenen Fußnoten des Musters bleiben; dazu eine mit einer inneren, frei erfunden.
        input.sections[3].body.push_str("\nDer Zaun steht.#footnote[Aus Lärche.#footnote[Vom Nordhang.]]\n");
        let book = build(&input).unwrap();
        let file = if mode == FootnoteMode::Popup { format!("text/k{ch2}.xhtml") } else { "text/notes.xhtml".into() };
        let notes = text(&book, &file);
        assert!(
            at(&notes, "Aus Lärche.") < at(&notes, "Vom Nordhang."),
            "{mode:?}: die äußere Fußnote steht vor der inneren: {notes}"
        );
        let outer = &notes[..at(&notes, "Aus Lärche.")];
        let outer_id = &outer[outer.rfind("id=\"fn").expect("Anker der äußeren")..];
        let inner = &notes[..at(&notes, "Vom Nordhang.")];
        let inner_id = &inner[inner.rfind(" id=\"fn").expect("Anker der inneren")..];
        let number = |s: &str| -> usize {
            s.split("id=\"fn").nth(1).unwrap().split('"').next().unwrap().parse().expect("Nummer im Anker")
        };
        assert_eq!(
            number(inner_id),
            number(outer_id) + 1,
            "{mode:?}: die innere Fußnote trägt die nächste Nummer nach der äußeren"
        );
    }
}

#[test]
fn a_reference_to_a_labelled_figure_shows_its_number_like_typst() {
    assert_eq!(
        figure_label("#figure(image(\"a.png\"), caption: [Der Teich]) <teich>"),
        Some(Some("teich".into())),
        "eine Abbildung mit Label nennt ihren Labelnamen, damit @teich auf ihre Nummer zeigen kann"
    );
    assert_eq!(figure_label("#figure(image(\"a.png\"))"), Some(None), "Abbildung ohne Label");
    assert_eq!(figure_label("#image(\"a.png\")"), None, "ein bloßes Bild ist keine Abbildung mit Nummer");
    assert_eq!(figure_label("#figure(image(\"a.png\")) <a b>"), Some(None), "kein gültiger Labelname");

    let (ch1, ch2) = (KEYS[1], KEYS[3]);
    let mut input = sample::input();
    // Das Muster hat schon eine Abbildung (`bilder/weide.png`); dazu eine zweite mit Label im zweiten Kapitel.
    input.sections[3].body.push_str(
        "\n#figure(image(\"bilder/plan.png\"), caption: [Der Lageplan]) <lageplan>\n\nSiehe @lageplan und \
         @lageplan[Skizze].\n",
    );
    let before: usize = input.sections[..3].iter().map(|s| s.body.matches("#figure(").count()).sum();
    let earlier =
        input.sections[3].body[..input.sections[3].body.find("<lageplan>").unwrap()].matches("#figure(").count();
    let expected = before + earlier;
    assert!(expected >= 2, "Testvoraussetzung: vor der neuen Abbildung steht mindestens eine andere im Buch");
    let book = build(&input).unwrap();
    let chapter = text(&book, &format!("text/k{ch2}.xhtml"));
    assert!(
        chapter.contains("<figure class=\"baruch-figure\" id=\"lbl-lageplan\">"),
        "die Abbildung trägt ihr Label als Anker: {chapter}"
    );
    assert!(
        chapter.contains(&format!(
            "<a class=\"baruch-ref\" href=\"k{ch2}.xhtml#lbl-lageplan\">Abbildung\u{a0}{expected}</a>"
        )),
        "`@lageplan` zeigt „Abbildung {expected}“ (laufende Nummer im Buch) statt des Labelnamens und verlinkt: {chapter}"
    );
    assert!(
        chapter.contains(&format!("#lbl-lageplan\">Skizze\u{a0}{expected}</a>")),
        "ein eigenes Wort am Verweis ersetzt „Abbildung“, die Nummer bleibt: {chapter}"
    );
    assert!(!chapter.contains(">lageplan<"), "der rohe Labelname steht nirgends als Linktext: {chapter}");
    assert!(
        !book.warnings.iter().any(|w| w.message.contains("lageplan")),
        "kein Hinweis zu diesem Verweis: {:?}",
        book.warnings
    );
    let _ = ch1;
}

/// Baut das Paket neu, mit `mimetype` in der gewünschten Methode und mit dem gewünschten Inhalt.
fn repack(epub: &[u8], method: CompressionMethod, mimetype: &[u8]) -> Vec<u8> {
    let mut zip = ZipArchive::new(Cursor::new(epub)).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        let (kind, content) =
            if name == "mimetype" { (method, mimetype.to_vec()) } else { (CompressionMethod::Deflated, bytes) };
        writer.start_file(name, SimpleFileOptions::default().compression_method(kind)).unwrap();
        writer.write_all(&content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn a_compressed_mimetype_with_wrong_content_reports_the_content_too() {
    let good = package(&build(&sample::input()).unwrap()).unwrap();
    let has = |epub: &[u8], code: &str| {
        check_structure(epub, None).0.iter().any(|f| f.code == code && f.severity == Severity::Error)
    };
    // Lang genug, dass DEFLATE wirklich komprimiert; kein gültiger Medientyp.
    let wrong = b"application/zip application/zip application/zip application/zip";
    let compressed_wrong = repack(&good, CompressionMethod::Deflated, wrong);
    assert!(has(&compressed_wrong, codes::MIMETYPE_COMPRESSED), "komprimiert bleibt ein Fehler (OCF verlangt STORED)");
    assert!(
        has(&compressed_wrong, codes::MIMETYPE_CONTENT),
        "der falsche Inhalt wird auch hinter der Kompression gefunden (vorher blieb er unentdeckt)"
    );
    let finding = check_structure(&compressed_wrong, None).0;
    let content = finding.iter().find(|f| f.code == codes::MIMETYPE_CONTENT).unwrap();
    assert!(content.message.contains("application/zip"), "die Meldung nennt den gefundenen Inhalt: {content:?}");

    let compressed_right = repack(&good, CompressionMethod::Deflated, b"application/epub+zip");
    assert!(has(&compressed_right, codes::MIMETYPE_COMPRESSED), "komprimiert mit richtigem Inhalt: nur dieser Fehler");
    assert!(
        !has(&compressed_right, codes::MIMETYPE_CONTENT),
        "richtiger Inhalt wird nicht als falsch gemeldet, auch wenn er komprimiert ist"
    );
    assert!(
        !has(&good, codes::MIMETYPE_CONTENT) && !has(&good, codes::MIMETYPE_COMPRESSED),
        "das Muster ist in Ordnung"
    );
}
