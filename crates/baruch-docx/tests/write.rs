// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writer (contract Round 32 §3, task docx_export): `write` yields a complete, well-formed and deterministic
//! archive according to `ooxml.rs`. It is checked with small reading helpers of its own (zip, quick-xml), not
//! with the reader of the neighbouring task. All texts are freely invented.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use baruch_docx::ooxml::*;
use baruch_docx::sample::{SAMPLE_CHANGE_DATE, SAMPLE_EDITOR, SAMPLE_PNG, sample_document};
use baruch_docx::write::{WriteOptions, write};
use baruch_docx::{Block, Change, ChangeKind, Comment, DocxDocument, DocxError, Inline, ListItem, Marks};
use quick_xml::Reader;
use quick_xml::events::Event;

fn options() -> WriteOptions {
    WriteOptions { created: "2026-11-25T08:00:00Z".into(), generator: "Baruch Probe".into() }
}

/// All entries of the archive in their order.
fn unpack(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("die Datei ist ein ZIP-Archiv");
    (0..zip.len())
        .map(|i| {
            let mut entry = zip.by_index(i).expect("Eintrag lesbar");
            let mut data = Vec::new();
            entry.read_to_end(&mut data).expect("Eintrag entpackbar");
            (entry.name().expect("Name des Eintrags lesbar").into_owned(), data)
        })
        .collect()
}

fn part(files: &[(String, Vec<u8>)], name: &str) -> String {
    let data = &files.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("Teil {name} fehlt im Archiv")).1;
    String::from_utf8(data.clone()).unwrap_or_else(|_| panic!("Teil {name} ist kein UTF-8"))
}

/// An element with its attributes (names including prefix, as written).
#[derive(Debug, Clone)]
struct Element {
    name: String,
    attrs: BTreeMap<String, String>,
}

impl Element {
    fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map_or("", String::as_str)
    }
}

/// Reads an XML all the way through: elements in order and the text per element name (entities resolved). Fails
/// loudly if the XML is not well-formed.
fn parse(name: &str, xml: &str) -> (Vec<Element>, Vec<(String, String)>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_end_names = true;
    let (mut elements, mut texts, mut stack) = (Vec::new(), Vec::<(String, String)>::new(), Vec::<String>::new());
    let push_text = |stack: &[String], text: &str, texts: &mut Vec<(String, String)>| {
        let Some(top) = stack.last() else { return };
        match texts.last_mut() {
            Some((element, existing)) if element == top => existing.push_str(text),
            _ => texts.push((top.clone(), text.to_owned())),
        }
    };
    loop {
        let event = reader.read_event().unwrap_or_else(|e| panic!("{name} ist nicht wohlgeformt: {e}"));
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                let element_name = start.name().into_inner().to_owned();
                let attrs = start
                    .attributes()
                    .map(|a| {
                        let a = a.unwrap_or_else(|e| panic!("{name}: Attribut unlesbar: {e}"));
                        let value = a
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .unwrap_or_else(|e| panic!("{name}: Attributwert: {e}"));
                        (a.key.into_inner().to_owned(), value.into_owned())
                    })
                    .collect();
                elements.push(Element { name: element_name.clone(), attrs });
                if matches!(event, Event::Start(_)) {
                    stack.push(element_name);
                    // Every text run starts anew, so that two runs do not merge into one text.
                    texts.push((stack.last().cloned().unwrap_or_default(), String::new()));
                }
            }
            Event::End(_) => {
                stack.pop().unwrap_or_else(|| panic!("{name}: schließendes Element ohne öffnendes"));
            }
            Event::Text(text) => push_text(&stack, AsRef::<str>::as_ref(&text), &mut texts),
            Event::GeneralRef(reference) => {
                let raw = AsRef::<str>::as_ref(&reference).to_owned();
                let resolved = match raw.as_str() {
                    "amp" => "&".to_owned(),
                    "lt" => "<".to_owned(),
                    "gt" => ">".to_owned(),
                    "quot" => "\"".to_owned(),
                    "apos" => "'".to_owned(),
                    other => {
                        let code = other
                            .strip_prefix("#x")
                            .map(|hex| u32::from_str_radix(hex, 16))
                            .or_else(|| other.strip_prefix('#').map(str::parse))
                            .unwrap_or_else(|| panic!("{name}: unbekannte Entität &{other};"))
                            .unwrap_or_else(|_| panic!("{name}: Zeichenreferenz &{other};"));
                        char::from_u32(code).expect("gültiges Zeichen").to_string()
                    }
                };
                push_text(&stack, &resolved, &mut texts);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert!(stack.is_empty(), "{name}: nicht alle Elemente sind geschlossen: {stack:?}");
    texts.retain(|(_, text)| !text.is_empty());
    (elements, texts)
}

fn elements(files: &[(String, Vec<u8>)], name: &str) -> Vec<Element> {
    parse(name, &part(files, name)).0
}

/// Texts of all elements `element` of a part.
fn texts_of(files: &[(String, Vec<u8>)], name: &str, element: &str) -> Vec<String> {
    parse(name, &part(files, name)).1.into_iter().filter(|(e, _)| e == element).map(|(_, t)| t).collect()
}

fn text(text: &str) -> Inline {
    Inline::Text { text: text.to_owned(), marks: Marks::default() }
}

fn paragraph(content: Vec<Inline>) -> Block {
    Block::Paragraph { content, mark: None }
}

fn document(blocks: Vec<Block>) -> DocxDocument {
    DocxDocument { blocks, ..DocxDocument::default() }
}

/// Styles (`w:pStyle`) of the paragraphs of a part in reading order; paragraphs without a style as „“.
fn paragraph_styles(files: &[(String, Vec<u8>)]) -> Vec<String> {
    let mut styles = Vec::new();
    for element in elements(files, PART_DOCUMENT) {
        match element.name.as_str() {
            "w:p" => styles.push(String::new()),
            "w:pStyle" => *styles.last_mut().expect("pStyle steht in einem Absatz") = element.attr("w:val").to_owned(),
            _ => {}
        }
    }
    styles
}

#[test]
fn the_sample_becomes_a_complete_well_formed_archive() {
    let bytes = write(&sample_document(), &options()).expect("das Beispiel lässt sich schreiben");
    let files = unpack(&bytes);
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    for wanted in [
        PART_CONTENT_TYPES,
        PART_ROOT_RELS,
        PART_DOCUMENT,
        PART_DOCUMENT_RELS,
        PART_STYLES,
        PART_SETTINGS,
        PART_FOOTNOTES,
        PART_NUMBERING,
        PART_COMMENTS,
        PART_CORE,
        PART_APP,
        "word/media/image1.png",
    ] {
        assert!(names.contains(&wanted), "Teil {wanted} gehört laut Vertrag §3.4 in die Datei; vorhanden: {names:?}");
    }
    assert_eq!(
        names[0], PART_CONTENT_TYPES,
        "[Content_Types].xml steht vorn – manche Programme erkennen daran das Paket"
    );
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "kein Teil steht doppelt im Archiv");
    for (name, data) in &files {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let xml = String::from_utf8(data.clone()).unwrap_or_else(|_| panic!("{name} ist kein UTF-8"));
            assert!(xml.starts_with("<?xml"), "{name} beginnt mit der XML-Deklaration");
            parse(name, &xml);
        }
    }
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    let first = write(&sample_document(), &options()).expect("schreiben");
    let second = write(&sample_document(), &options()).expect("schreiben");
    assert_eq!(first, second, "gleiche Eingabe und gleiche Angaben ergeben dieselben Bytes (feste ZIP-Zeiten)");
    let later = write(&sample_document(), &WriteOptions { created: "2026-11-26T08:00:00Z".into(), ..options() })
        .expect("schreiben");
    assert_ne!(
        first, later,
        "der Zeitpunkt aus den Angaben steht wirklich in der Datei (core.xml), sonst wäre er wirkungslos"
    );
}

#[test]
fn every_used_style_is_defined_and_every_relationship_resolves() {
    let files = unpack(&write(&sample_document(), &options()).expect("schreiben"));
    let defined: BTreeSet<String> = elements(&files, PART_STYLES)
        .into_iter()
        .filter(|e| e.name == "w:style")
        .map(|e| e.attr("w:styleId").to_owned())
        .collect();
    for name in [PART_DOCUMENT, PART_FOOTNOTES, PART_COMMENTS] {
        for element in elements(&files, name) {
            if element.name == "w:pStyle" || element.name == "w:rStyle" {
                let style = element.attr("w:val");
                assert!(
                    defined.contains(style),
                    "{name} benutzt die Vorlage „{style}“, styles.xml definiert sie nicht"
                );
            }
        }
    }
    for style in [
        "Title",
        "Subtitle",
        "Heading1",
        "Heading2",
        "Heading3",
        "BaruchTeil",
        "BaruchOhneNummer1",
        "BaruchOhneNummer3",
        "BaruchAutor",
        "BaruchImpressum",
        "Quote",
        "BaruchZitatquelle",
        "BaruchZitat",
        "Caption",
        "ListParagraph",
        "BaruchTypst",
        "BaruchTypstZeichen",
        "FootnoteText",
        "FootnoteReference",
    ] {
        assert!(defined.contains(style), "die Vorlage „{style}“ aus ooxml.rs ist in jeder Datei definiert");
    }
    let names: Vec<String> = elements(&files, PART_STYLES)
        .into_iter()
        .filter(|e| e.name == "w:name")
        .map(|e| e.attr("w:val").to_owned())
        .collect();
    assert!(
        names.contains(&"heading 1".to_owned()) && names.contains(&"footnote text".to_owned()),
        "eingebaute Vorlagen tragen ihren eingebauten Namen – daran zeigt Word sie deutsch als „Überschrift 1“: {names:?}"
    );

    let relations: BTreeMap<String, Element> = elements(&files, PART_DOCUMENT_RELS)
        .into_iter()
        .filter(|e| e.name == "Relationship")
        .map(|e| (e.attr("Id").to_owned(), e))
        .collect();
    let mut used = 0;
    for element in elements(&files, PART_DOCUMENT) {
        for key in ["r:id", "r:embed"] {
            if let Some(id) = element.attrs.get(key) {
                used += 1;
                assert!(
                    relations.contains_key(id),
                    "{key}=\"{id}\" im Dokument hat keine Beziehung in document.xml.rels"
                );
            }
        }
    }
    assert!(used >= 1, "das Beispiel hat ein Bild, also mindestens eine benutzte Beziehung");
    let names: BTreeSet<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    for relation in relations.values() {
        if relation.attr("TargetMode") != TARGET_MODE_EXTERNAL {
            let target = format!("word/{}", relation.attr("Target"));
            assert!(names.contains(target.as_str()), "das Ziel {target} einer Beziehung liegt im Archiv");
        }
    }
    for relation in elements(&files, PART_ROOT_RELS).into_iter().filter(|e| e.name == "Relationship") {
        assert!(
            names.contains(relation.attr("Target")),
            "Ziel {} aus _rels/.rels liegt im Archiv",
            relation.attr("Target")
        );
    }
    // Every part has a content type: as an override or through the extension.
    let types = elements(&files, PART_CONTENT_TYPES);
    for name in names.iter().filter(|n| **n != PART_CONTENT_TYPES) {
        let extension = name.rsplit_once('.').map_or("", |(_, e)| e);
        let known = types.iter().any(|e| {
            (e.name == "Override" && e.attr("PartName") == format!("/{name}"))
                || (e.name == "Default" && e.attr("Extension") == extension)
        });
        assert!(
            known,
            "für {name} steht kein Inhaltstyp in [Content_Types].xml – Word meldete die Datei als beschädigt"
        );
    }
}

#[test]
fn headings_parts_and_unnumbered_headings_carry_their_styles() {
    let heading = |level: u8, numbered: bool, title: &str| Block::Heading {
        level,
        numbered,
        content: vec![text(title)],
        mark: None,
    };
    let doc = document(vec![
        Block::Part { content: vec![text("Erster Teil")], mark: None },
        heading(1, true, "Kapitel"),
        heading(2, true, "Abschnitt"),
        heading(3, true, "Unterabschnitt"),
        heading(1, false, "Vorwort"),
        heading(3, false, "Dank"),
        heading(4, true, "Zwischentitel"),
        paragraph(vec![text("Fließtext ohne Vorlage.")]),
    ]);
    let files = unpack(&write(&doc, &options()).expect("schreiben"));
    assert_eq!(
        paragraph_styles(&files),
        vec![
            "BaruchTeil",
            "Heading1",
            "Heading2",
            "Heading3",
            "BaruchOhneNummer1",
            "BaruchOhneNummer3",
            "Heading4",
            ""
        ],
        "Teil, Überschrift 1–3, ohne Nummer und Überschrift 4 tragen die Vorlagen aus ooxml.rs; Fließtext bleibt Standard"
    );
    assert_eq!(
        texts_of(&files, PART_DOCUMENT, "w:t"),
        vec![
            "Erster Teil",
            "Kapitel",
            "Abschnitt",
            "Unterabschnitt",
            "Vorwort",
            "Dank",
            "Zwischentitel",
            "Fließtext ohne Vorlage."
        ],
        "die Texte stehen in Lesereihenfolge und vollständig da"
    );
}

#[test]
fn front_matter_comes_from_the_meta_data_and_is_repeated_in_core_properties() {
    let files = unpack(&write(&sample_document(), &options()).expect("schreiben"));
    let styles = paragraph_styles(&files);
    assert_eq!(
        &styles[..5],
        ["Title", "Subtitle", "BaruchAutor", "BaruchImpressum", "BaruchImpressum"],
        "die Titelei steht vorn: Titel, Untertitel, Autor und je Zeile des Impressums ein Absatz"
    );
    let texts = texts_of(&files, PART_DOCUMENT, "w:t");
    assert_eq!(
        &texts[..5],
        [
            "Der Garten hinter der Mühle",
            "Eine erfundene Erzählung",
            "Hanna Probst",
            "© 2026 Hanna Probst",
            "Erfundenes Beispiel für die Tests von Baruch"
        ],
        "mit dem Wortlaut aus DocxMeta"
    );
    let core = parse(PART_CORE, &part(&files, PART_CORE)).1;
    let value = |tag: &str| core.iter().find(|(e, _)| e == tag).map(|(_, t)| t.clone()).unwrap_or_default();
    assert_eq!(value("dc:title"), "Der Garten hinter der Mühle", "Titel in den Dateieigenschaften");
    assert_eq!(value("dc:subject"), "Eine erfundene Erzählung", "Untertitel als Betreff");
    assert_eq!(value("dc:creator"), "Hanna Probst", "Autorin");
    assert_eq!(value("dc:language"), "de", "Sprache wie in den Werkangaben");
    assert!(value("dc:description").contains("© 2026 Hanna Probst"), "Impressum in der Beschreibung");
    assert_eq!(value("dcterms:created"), "2026-11-25T08:00:00Z", "Zeitpunkt aus den Angaben des Aufrufers");
    let app = parse(PART_APP, &part(&files, PART_APP)).1;
    assert!(
        app.contains(&("Application".to_owned(), "Baruch Probe".to_owned())),
        "erzeugendes Programm in app.xml: {app:?}"
    );
    let language =
        elements(&files, PART_STYLES).into_iter().find(|e| e.name == "w:lang").expect("w:lang in styles.xml");
    assert_eq!(language.attr("w:val"), "de-DE", "Word erwartet Sprache und Land; „de“ wird zu de-DE");

    let empty = unpack(
        &write(&document(vec![paragraph(vec![text("Nur Text.")])]), &WriteOptions::default()).expect("schreiben"),
    );
    assert_eq!(paragraph_styles(&empty), vec![""], "ohne Werkangaben entsteht keine leere Titelei");
    assert!(
        !part(&empty, PART_CORE).contains("dcterms:created"),
        "ohne Zeitpunkt fehlt die Angabe, statt ungültig zu sein"
    );
}

#[test]
fn footnotes_are_real_footnotes() {
    let doc = document(vec![paragraph(vec![
        text("Erster Satz."),
        Inline::Footnote {
            content: vec![
                text("Erste Anmerkung mit "),
                Inline::Link { url: "https://example.org/a?b=1&c=2".into(), content: vec![text("Verweis")] },
            ],
        },
        text(" Zweiter Satz."),
        Inline::Footnote { content: vec![text("Zweite Anmerkung")] },
    ])]);
    let files = unpack(&write(&doc, &options()).expect("schreiben"));
    let references: Vec<String> = elements(&files, PART_DOCUMENT)
        .into_iter()
        .filter(|e| e.name == "w:footnoteReference")
        .map(|e| e.attr("w:id").to_owned())
        .collect();
    assert_eq!(references, vec!["2", "3"], "zwei Fußnotenzeichen im Text; 0 und 1 sind die Trennlinien");
    let notes: Vec<String> = elements(&files, PART_FOOTNOTES)
        .into_iter()
        .filter(|e| e.name == "w:footnote")
        .map(|e| e.attr("w:id").to_owned())
        .collect();
    assert_eq!(notes, vec!["0", "1", "2", "3"], "zu jedem Zeichen gibt es den Eintrag in footnotes.xml");
    let body = texts_of(&files, PART_DOCUMENT, "w:t");
    assert_eq!(body, vec!["Erster Satz.", " Zweiter Satz."], "der Text der Fußnoten steht nicht im Fließtext");
    let note_texts = texts_of(&files, PART_FOOTNOTES, "w:t").join("|");
    assert!(
        note_texts.contains("Erste Anmerkung mit |Verweis") && note_texts.contains("Zweite Anmerkung"),
        "sondern in footnotes.xml: {note_texts}"
    );
    let relations = elements(&files, "word/_rels/footnotes.xml.rels");
    let link = relations.iter().find(|e| e.name == "Relationship").expect("Beziehung des Verweises in der Fußnote");
    assert_eq!(
        link.attr("Target"),
        "https://example.org/a?b=1&c=2",
        "die Adresse steht unverändert (im XML maskiert) da"
    );
    assert_eq!(link.attr("TargetMode"), "External", "Adressen sind äußere Ziele");
    let settings = elements(&files, PART_SETTINGS);
    assert_eq!(
        settings.iter().filter(|e| e.name == "w:footnote").count(),
        2,
        "settings.xml nennt die beiden Trennlinien, sonst zeichnet Word keine Linie über den Fußnoten"
    );
}

#[test]
fn images_are_embedded_with_alt_text_and_caption() {
    let files = unpack(&write(&sample_document(), &options()).expect("schreiben"));
    let media = &files.iter().find(|(n, _)| n == "word/media/image1.png").expect("Bild im Archiv").1;
    assert_eq!(media.as_slice(), SAMPLE_PNG, "die Bytes des Bildes liegen unverändert unter word/media/");
    let all = elements(&files, PART_DOCUMENT);
    let properties = all.iter().find(|e| e.name == "wp:docPr").expect("wp:docPr der Abbildung");
    assert_eq!(properties.attr("descr"), "Ein einzelner grauer Bildpunkt", "Alternativtext in wp:docPr descr");
    assert_eq!(
        properties.attr("name"),
        "bilder/garten.png",
        "der Pfad im Werk steht im Namen – so findet ihn der Leser wieder"
    );
    let blip = all.iter().find(|e| e.name == "a:blip").expect("a:blip");
    let relation = elements(&files, PART_DOCUMENT_RELS)
        .into_iter()
        .find(|e| e.attr("Id") == blip.attr("r:embed"))
        .expect("Beziehung des Bildes");
    assert_eq!(relation.attr("Target"), "media/image1.png", "die Beziehung zeigt auf die eingebettete Datei");
    assert_eq!(relation.attr("Type"), REL_IMAGE, "mit der Beziehungsart für Bilder");
    let styles = paragraph_styles(&files);
    let texts = texts_of(&files, PART_DOCUMENT, "w:t");
    assert!(
        styles.contains(&"Caption".to_owned()) && texts.contains(&"Der Garten im Frühjahr".to_owned()),
        "die Bildunterschrift steht als eigener Absatz mit der Vorlage Caption"
    );
    let types = elements(&files, PART_CONTENT_TYPES);
    assert!(
        types
            .iter()
            .any(|e| e.name == "Default" && e.attr("Extension") == "png" && e.attr("ContentType") == "image/png"),
        "der Inhaltstyp des Bildes ist angemeldet"
    );

    let missing =
        document(vec![Block::Figure { image: 3, alt: String::new(), caption: Vec::new(), raw: String::new() }]);
    assert!(
        matches!(write(&missing, &options()), Err(DocxError::Invalid(_))),
        "eine Abbildung ohne Bild im Dokument ist ein Fehler des Aufrufers, keine kaputte Datei"
    );
}

#[test]
fn tracked_changes_and_comments_of_the_sample_are_written() {
    let files = unpack(&write(&sample_document(), &options()).expect("schreiben"));
    let all = elements(&files, PART_DOCUMENT);
    let deletion = all.iter().find(|e| e.name == "w:del").expect("w:del der Löschung");
    let insertion = all.iter().find(|e| e.name == "w:ins").expect("w:ins der Einfügung");
    for change in [deletion, insertion] {
        assert_eq!(change.attr("w:author"), SAMPLE_EDITOR, "die Änderung nennt die Bearbeiterin wörtlich");
        assert_eq!(change.attr("w:date"), SAMPLE_CHANGE_DATE, "und den Zeitpunkt");
    }
    assert_ne!(deletion.attr("w:id"), insertion.attr("w:id"), "jede Änderung hat ihre eigene Kennung");
    assert_eq!(texts_of(&files, PART_DOCUMENT, "w:delText"), vec!["schmal"], "gelöschter Text steht als w:delText");
    let visible = texts_of(&files, PART_DOCUMENT, "w:t");
    assert!(visible.contains(&"breit".to_owned()), "eingefügter Text ist gewöhnlicher Text in w:ins");
    assert!(!visible.contains(&"schmal".to_owned()), "gelöschter Text steht nicht zusätzlich als sichtbarer Text da");

    let names: Vec<&str> = all.iter().map(|e| e.name.as_str()).collect();
    let position = |name: &str| names.iter().position(|n| *n == name).unwrap_or_else(|| panic!("{name} fehlt"));
    assert!(
        position("w:commentRangeStart") < position("w:commentRangeEnd")
            && position("w:commentRangeEnd") < position("w:commentReference"),
        "der Kommentar ist verankert: Anfang, Ende, dann das Kommentarzeichen"
    );
    let comment = elements(&files, PART_COMMENTS).into_iter().find(|e| e.name == "w:comment").expect("Kommentar");
    assert_eq!(comment.attr("w:id"), "0", "Kennung wie im Anker");
    assert_eq!(comment.attr("w:author"), SAMPLE_EDITOR, "Verfasserin des Kommentars");
    assert_eq!(comment.attr("w:initials"), "EB", "mit Kürzel");
    assert_eq!(
        texts_of(&files, PART_COMMENTS, "w:t"),
        vec!["Ist die Farbe des Zauns für die Geschichte wichtig?"],
        "der Wortlaut des Kommentars steht in comments.xml"
    );
}

#[test]
fn paragraph_marks_replies_and_loose_comments() {
    let change = |kind| Change { kind, author: "@lektor".to_owned(), date: "kein Datum".to_owned() };
    let mut doc = document(vec![
        Block::Paragraph {
            content: vec![Inline::Tracked { change: change(ChangeKind::Insert), content: vec![text("Neuer Absatz.")] }],
            mark: Some(change(ChangeKind::Insert)),
        },
        paragraph(vec![
            Inline::CommentStart { id: 7 },
            text("Stelle"),
            Inline::CommentEnd { id: 7 },
            Inline::CommentEnd { id: 99 },
        ]),
    ]);
    let comment = |id, text: &str, reply_to| Comment {
        id,
        author: "Erika Beispiel".into(),
        initials: String::new(),
        date: String::new(),
        text: text.into(),
        reply_to,
    };
    doc.comments = vec![comment(7, "Frage", None), comment(8, "Antwort", Some(7)), comment(9, "Ohne Stelle", None)];
    let files = unpack(&write(&doc, &options()).expect("schreiben"));
    let xml = part(&files, PART_DOCUMENT);
    assert!(
        xml.contains("<w:pPr><w:rPr><w:ins w:id=\"1\" w:author=\"@lektor\"/></w:rPr></w:pPr>"),
        "die Absatzmarke eines neuen Absatzes ist als eingefügt verfolgt; ein unlesbares Datum entfällt, statt die Datei ungültig zu machen: {xml}"
    );
    let all = elements(&files, PART_DOCUMENT);
    let ids = |name: &str| -> Vec<String> {
        all.iter().filter(|e| e.name == name).map(|e| e.attr("w:id").to_owned()).collect()
    };
    assert_eq!(
        ids("w:commentRangeStart"),
        vec!["7", "8"],
        "die Antwort ist an der Stelle ihres Kommentars mit verankert"
    );
    assert_eq!(
        ids("w:commentReference"),
        vec!["7", "8", "9"],
        "jeder Kommentar hat genau ein Zeichen im Text – der ohne Stelle am Schluss; ein Anker ohne Kommentar (99) entfällt"
    );
}

#[test]
fn typst_islands_keep_their_source_hidden_and_forbidden_characters_are_replaced() {
    let doc = document(vec![
        Block::Typst {
            raw: "#let breite = 3cm\n#rect(width: breite)".into(),
            shown: vec![paragraph(vec![text("[Rechteck]")])],
        },
        paragraph(vec![
            text("Formel "),
            Inline::Typst { raw: "$a < b$".into(), shown: vec![text("a < b")] },
            text(" und ein Steuerzeichen\u{1}\tmit Tabulator."),
        ]),
    ]);
    let files = unpack(&write(&doc, &options()).expect("schreiben"));
    let all = elements(&files, PART_DOCUMENT);
    let tags: Vec<&str> = all.iter().filter(|e| e.name == "w:tag").map(|e| e.attr("w:val")).collect();
    assert_eq!(tags, vec![SDT_TAG_TYPST, SDT_TAG_TYPST], "Block und Insel im Text tragen das Kennzeichen aus ooxml.rs");
    assert_eq!(all.iter().filter(|e| e.name == "w:sdt").count(), 2, "je Insel ein Inhaltssteuerelement");
    assert!(
        all.iter().filter(|e| e.name == "w:vanish").count() >= 3,
        "der Quelltext ist verborgen (Absatzmarke und Läufe)"
    );
    let texts = texts_of(&files, PART_DOCUMENT, "w:t");
    assert_eq!(
        texts,
        vec![
            "#let breite = 3cm",
            "#rect(width: breite)",
            "[Rechteck]",
            "Formel ",
            "$a < b$",
            "a < b",
            " und ein Steuerzeichen\u{fffd}",
            "mit Tabulator."
        ],
        "der Quelltext steht wörtlich da (Zeilen durch w:br getrennt), dahinter das Sichtbare; das Steuerzeichen ist ersetzt"
    );
    assert_eq!(all.iter().filter(|e| e.name == "w:br").count(), 1, "der Zeilenumbruch im Quelltext ist ein w:br");
    assert_eq!(all.iter().filter(|e| e.name == "w:tab").count(), 1, "der Tabulator ein w:tab");
}

#[test]
fn lists_tables_quotes_and_page_breaks() {
    let item = |depth, content: &str| ListItem { depth, content: vec![text(content)], mark: None };
    let doc = document(vec![
        Block::List { ordered: true, start: 1, items: vec![item(0, "eins"), item(1, "eins a")] },
        Block::List { ordered: false, start: 1, items: vec![item(0, "Punkt")] },
        Block::List { ordered: true, start: 4, items: vec![item(0, "vier")] },
        Block::Quote { blocks: vec![paragraph(vec![text("Zitat.")])], attribution: vec![text("Quelle")] },
        paragraph(vec![
            Inline::Quote { content: vec![text("im Text")] },
            Inline::LineBreak,
            Inline::Text { text: "fett".into(), marks: Marks { bold: true, italic: true, ..Marks::default() } },
        ]),
        Block::PageBreak,
        Block::Table {
            header_rows: 1,
            rows: vec![
                vec![vec![paragraph(vec![text("Sorte")])], vec![paragraph(vec![text("Zeit")])]],
                vec![vec![paragraph(vec![text("Bohne")])], vec![]],
            ],
        },
    ]);
    let files = unpack(&write(&doc, &options()).expect("schreiben"));
    let all = elements(&files, PART_DOCUMENT);
    let numbers: Vec<&str> = all.iter().filter(|e| e.name == "w:numId").map(|e| e.attr("w:val")).collect();
    assert_eq!(
        numbers,
        vec!["2", "2", "1", "3"],
        "geordnet = NUM_DECIMAL, ungeordnet = NUM_BULLET; die zweite geordnete Liste zählt für sich (eigene Nummerierung)"
    );
    let levels: Vec<&str> = all.iter().filter(|e| e.name == "w:ilvl").map(|e| e.attr("w:val")).collect();
    assert_eq!(levels, vec!["0", "1", "0", "0"], "die Tiefe eines Punkts ist die Listenebene");
    let numbering = elements(&files, PART_NUMBERING);
    let start = numbering.iter().find(|e| e.name == "w:startOverride").expect("Beginn der zweiten Liste");
    assert_eq!(start.attr("w:val"), "4", "die Liste beginnt bei ihrer ersten Nummer");
    let defined: Vec<&str> = numbering.iter().filter(|e| e.name == "w:num").map(|e| e.attr("w:numId")).collect();
    assert_eq!(defined, vec!["1", "2", "3"], "jede benutzte Nummerierung ist in numbering.xml definiert");

    assert_eq!(
        paragraph_styles(&files),
        vec![
            "ListParagraph",
            "ListParagraph",
            "ListParagraph",
            "ListParagraph",
            "Quote",
            "BaruchZitatquelle",
            "",
            "",
            "",
            "",
            "",
            ""
        ],
        "Listen, Blockzitat und Quelle tragen ihre Vorlagen; danach Absatz, Seitenumbruch und vier Zellen (die leere mit leerem Absatz)"
    );
    let run_styles: Vec<&str> = all.iter().filter(|e| e.name == "w:rStyle").map(|e| e.attr("w:val")).collect();
    assert_eq!(run_styles, vec!["BaruchZitat"], "das Zitat im Text trägt die Zeichenvorlage");
    assert_eq!(all.iter().filter(|e| e.name == "w:br" && e.attr("w:type") == "page").count(), 1, "ein Seitenumbruch");
    assert_eq!(all.iter().filter(|e| e.name == "w:br" && e.attr("w:type").is_empty()).count(), 1, "ein Zeilenumbruch");
    assert_eq!(all.iter().filter(|e| e.name == "w:b").count(), 1, "fett");
    assert_eq!(all.iter().filter(|e| e.name == "w:i").count(), 1, "kursiv");
    assert_eq!(all.iter().filter(|e| e.name == "w:tr").count(), 2, "zwei Tabellenzeilen");
    assert_eq!(all.iter().filter(|e| e.name == "w:tc").count(), 4, "vier Zellen, auch die leere");
    assert_eq!(all.iter().filter(|e| e.name == "w:tblHeader").count(), 1, "die erste Zeile ist Kopfzeile");
    assert_eq!(all.iter().filter(|e| e.name == "w:gridCol").count(), 2, "zwei Spalten im Raster");
}
