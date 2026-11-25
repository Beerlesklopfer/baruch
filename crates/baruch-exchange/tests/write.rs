// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26 „export“: Schreiben von `werk.xml`, `MANIFEST.xml` und dem Container `.baruch` (Vertrag F1/F2).
//!
//! Zurückgelesen wird mit quick-xml ohne `trim_text` (nicht mit dem Leser von import), so wie ein beliebiges
//! anderes Programm die Datei läse: Zeichenreferenzen und die fünf vordefinierten Entitäten werden aufgelöst,
//! sonst nichts.

use std::io::Read as _;

use baruch_exchange::sample::{SAMPLE_CONTROL_BODY, SAMPLE_TRICKY_BODY, sample_document, sample_uuid};
use baruch_exchange::write::{ImageMode, write_baruch, write_manifest, write_xml};
use baruch_exchange::*;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use quick_xml::events::Event;

/// Ein Element der Ausgabe: Pfad ab `werk`, Attribute (unmaskiert), Text (unmaskiert) und Elternteil.
#[derive(Debug)]
struct Element {
    path: String,
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    parent: Option<usize>,
}

impl Element {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

/// Liest XML mit quick-xml (ohne trim) in eine flache Liste; Text = Text, Referenzen und CDATA aneinander.
fn parse(xml: &str) -> Vec<Element> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut elements: Vec<Element> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        let event = reader.read_event().expect("wohlgeformtes XML");
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = e.local_name().into_inner().to_owned();
                let parent = stack.last().copied();
                let path = match parent {
                    Some(p) => format!("{}/{}", elements[p].path, name),
                    None => name.clone(),
                };
                let attrs = e
                    .attributes()
                    .map(|a| {
                        let a = a.unwrap();
                        (
                            a.key.into_inner().to_owned(),
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap().into_owned(),
                        )
                    })
                    .collect();
                elements.push(Element { path, name, attrs, text: String::new(), parent });
                if matches!(event, Event::Start(_)) {
                    stack.push(elements.len() - 1);
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(t) => {
                if let Some(&top) = stack.last() {
                    elements[top].text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref().unwrap() {
                    Some(c) => c.to_string(),
                    None => {
                        quick_xml::escape::resolve_predefined_entity(&r).expect("nur vordefinierte Entitäten").into()
                    }
                };
                if let Some(&top) = stack.last() {
                    elements[top].text.push_str(&resolved);
                }
            }
            Event::CData(c) => {
                if let Some(&top) = stack.last() {
                    elements[top].text.push_str(&c);
                }
            }
            Event::DocType(_) => panic!("Der Schreiber schreibt nie eine DTD"),
            Event::PI(_) => panic!("Der Schreiber schreibt keine Verarbeitungsanweisung"),
            Event::Eof => break,
            _ => {}
        }
    }
    elements
}

fn children<'a>(elements: &'a [Element], parent: usize, name: &str) -> Vec<&'a Element> {
    elements.iter().filter(|e| e.parent == Some(parent) && e.name == name).collect()
}

/// Inhalt eines Textelements, Base64 nach `kodierung` aufgelöst.
fn decoded(element: &Element) -> String {
    match element.attr("kodierung") {
        Some("base64") => String::from_utf8(STANDARD.decode(&element.text).unwrap()).unwrap(),
        Some(other) => panic!("unbekannte Kodierung {other}"),
        None => element.text.clone(),
    }
}

fn index_of(elements: &[Element], path: &str, uuid: &str) -> usize {
    elements
        .iter()
        .position(|e| e.path.ends_with(path) && e.attr("uuid") == Some(uuid))
        .unwrap_or_else(|| panic!("{path} mit uuid {uuid} fehlt"))
}

fn revision_text(elements: &[Element], uuid: &str) -> (String, bool) {
    let at = index_of(elements, "punkt/fassung", uuid);
    let text = children(elements, at, "text");
    assert_eq!(text.len(), 1, "genau ein <text> je Fassung");
    (decoded(text[0]), text[0].attr("kodierung").is_some())
}

#[test]
fn xml_is_deterministic_and_zip_too() {
    let doc = sample_document();
    // Dasselbe Dokument ergibt byte-gleiche Ausgaben (Vertrag: deterministisch, Zeitstempel im ZIP fest).
    assert_eq!(write_xml(&doc, ImageMode::Inline).unwrap(), write_xml(&doc, ImageMode::Inline).unwrap(), "XML Inline");
    assert_eq!(write_xml(&doc, ImageMode::Files).unwrap(), write_xml(&doc, ImageMode::Files).unwrap(), "XML Files");
    let zip = write_baruch(&doc).unwrap();
    assert_eq!(zip, write_baruch(&doc).unwrap(), "ZIP byte-gleich");
    // Kein Datum der Uhr im ZIP: jeder Eintrag trägt den festen Zeitstempel 1980-01-01 00:00:00.
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        let time = entry.last_modified().unwrap();
        assert_eq!(
            (time.year(), time.month(), time.day()),
            (1980, 1, 1),
            "fester Zeitstempel: {}",
            entry.name().unwrap()
        );
    }
}

#[test]
fn texts_come_back_exactly_without_trim() {
    let doc = sample_document();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    let elements = parse(&xml);
    // Die aktuelle Fassung von „Der Eimer“ (Leerzeichen vorn und hinten, Tab, <&>, ]]>, Umlaute, \r) kommt
    // gleich heraus und steht nicht als Base64 – sie hat kein verbotenes Zeichen.
    let (tricky, base64) = revision_text(&elements, &sample_uuid(111));
    assert_eq!(tricky, SAMPLE_TRICKY_BODY, "Text exakt, ohne trim");
    assert!(!base64, "ohne verbotenes Zeichen kein Base64");
    // \r steht als Zeichenreferenz; ein Parser normalisierte ein wörtliches \r\n sonst zu \n (XML 1.0 §2.11).
    assert!(xml.contains("&#13;"), "\\r als &#13;");
    assert!(!xml.contains('\r'), "kein wörtliches \\r in der Ausgabe");
    // ]]> darf im Inhalt nicht wörtlich stehen (XML 1.0 §2.4).
    assert!(!xml.contains("]]>"), "]]> ist maskiert");
    // Die Fassung mit U+0001 steht als Base64 mit kodierung="base64" und kommt ebenso exakt zurück.
    let (control, base64) = revision_text(&elements, &sample_uuid(113));
    assert!(base64, "verbotenes Steuerzeichen → kodierung=\"base64\"");
    assert_eq!(control, SAMPLE_CONTROL_BODY, "Base64 trägt den Text exakt");
    assert!(!xml.contains('\u{1}'), "U+0001 nie wörtlich im XML");
    // Alle übrigen Fassungen sind ebenso exakt (Basis, Status, Autor bleiben an der richtigen Fassung).
    let bucket = &doc.work.outline[0].children[0];
    for revision in &bucket.revisions {
        assert_eq!(revision_text(&elements, &revision.uuid).0, revision.body, "Fassung {}", revision.number);
    }
    let proposal = &elements[index_of(&elements, "punkt/fassung", &sample_uuid(113))];
    assert_eq!(proposal.attr("autor-art"), Some("assistent"), "Vorschlag vom Assistenten");
    assert_eq!(proposal.attr("modell"), Some("probe/modell-7b"), "Modell der Fassung als Herkunft");
    assert_eq!(proposal.attr("basis").map(str::to_owned), Some(sample_uuid(111)), "basis über die UUID");
    assert_eq!(proposal.attr("status"), Some("vorschlag"), "Status als XML-Wort");
    // Werkangaben mit Umlauten und &.
    let blurb = elements.iter().find(|e| e.path == "werk/buch/klappentext").unwrap();
    assert_eq!(blurb.text, "Ein Buch über Wasser & Licht.", "Klappentext exakt");
}

#[test]
fn attributes_keep_tabs_and_line_breaks() {
    let mut doc = sample_document();
    // Ein Autor mit Tab, Zeilenumbruch und \r im Namen: ohne Zeichenreferenz würde ein Parser die drei
    // Zeichen zu Leerzeichen normalisieren (XML 1.0 §3.3.3).
    doc.work.outline[0].children[0].posts[0].author = "Hanna\tProbe\nund\rFreunde \"<&>\"".into();
    let xml = write_xml(&doc, ImageMode::Files).unwrap();
    assert!(
        xml.contains("Hanna&#9;Probe&#10;und&#13;Freunde &quot;&lt;&amp;&gt;&quot;"),
        "Zeichenreferenzen im Attribut"
    );
    let elements = parse(&xml);
    let post = &elements[index_of(&elements, "punkt/beitrag", &sample_uuid(30))];
    assert_eq!(post.attr("autor"), Some("Hanna\tProbe\nund\rFreunde \"<&>\""), "Attribut exakt zurück");
}

#[test]
fn forbidden_character_in_an_attribute_names_the_field() {
    let mut doc = sample_document();
    // Attribute kennen kein Base64: ein verbotenes Zeichen ist ein Fehler mit dem Namen des Felds.
    doc.work.outline[0].children[0].revisions[1].author = ExchangeAuthor::Human { name: "Hanna\u{1}".into() };
    let error = write_xml(&doc, ImageMode::Inline).unwrap_err();
    assert!(matches!(error, ExchangeError::Write(_)), "ExchangeError::Write: {error:?}");
    let message = error.to_string();
    assert!(message.contains("fassung/@autor"), "Feldname in der Meldung: {message}");
    assert!(message.contains("U+0001"), "das Zeichen in der Meldung: {message}");
    // Auch ZIP und Manifest schreiben dann nichts.
    assert!(matches!(write_baruch(&doc), Err(ExchangeError::Write(_))), "write_baruch scheitert ebenso");
    let bad = [ExchangeManifestEntry { path: "werk\u{2}.xml".into(), sha256: sha256_hex(b""), bytes: 0 }];
    let message = write_manifest(&bad).unwrap_err().to_string();
    assert!(message.contains("datei/@pfad"), "Feld im Manifest: {message}");
}

#[test]
fn invalid_identifiers_and_images_are_refused() {
    // Eine nicht kanonische UUID ergäbe eine Datei, die das XSD ablehnt: lieber ein ehrlicher Fehler.
    let mut doc = sample_document();
    doc.work.outline[1].uuid = "26000000-0000-4000-8000-00000000001A".into();
    let message = write_xml(&doc, ImageMode::Inline).unwrap_err().to_string();
    assert!(message.contains("punkt/@uuid"), "Feld der falschen UUID: {message}");
    // Ein Bildtyp außerhalb des Formats und eine Prüfsumme, die nicht zu den Bytes passt.
    let mut doc = sample_document();
    doc.work.assets[0].revisions[0].mime = "image/bmp".into();
    assert!(matches!(write_xml(&doc, ImageMode::Files), Err(ExchangeError::Write(_))), "image/bmp");
    let mut doc = sample_document();
    doc.work.assets[0].revisions[0].data.push(0);
    assert!(write_baruch(&doc).unwrap_err().to_string().contains("sha256"), "Prüfsumme muss passen");
    // Ein leeres Manifest wäre nach dem XSD ungültig (datei+).
    assert!(matches!(write_manifest(&[]), Err(ExchangeError::Write(_))), "Manifest ohne Einträge");
}

#[test]
fn inline_xml_carries_images_as_base64_and_no_file_reference() {
    let doc = sample_document();
    let elements = parse(&write_xml(&doc, ImageMode::Inline).unwrap());
    for revision in doc.work.assets.iter().flat_map(|a| &a.revisions) {
        let at = index_of(&elements, "bild/fassung", &revision.uuid);
        // Nur XML: genau <daten> mit den Rohbytes, kein Verweis datei (genau eines von beiden, F2/Abschnitt 4).
        assert_eq!(elements[at].attr("datei"), None, "kein datei= im Nur-XML");
        let data = children(&elements, at, "daten");
        assert_eq!(data.len(), 1, "genau ein <daten>");
        assert_eq!(STANDARD.decode(&data[0].text).unwrap(), revision.data, "Base64 trägt die Bytes");
        assert_eq!(elements[at].attr("bytes"), Some(revision.data.len().to_string().as_str()), "bytes");
        assert_eq!(elements[at].attr("sha256"), Some(revision.sha256.as_str()), "sha256");
    }
    // Der Grabstein steht mit Art und Zeit in <geloescht>.
    let stone = elements.iter().find(|e| e.path == "werk/geloescht/eintrag").unwrap();
    assert_eq!(stone.attr("art"), Some("punkt"), "Grabstein eines Punkts");
    assert_eq!(stone.attr("uuid").map(str::to_owned), Some(sample_uuid(70)), "Grabstein mit seiner UUID");
}

#[test]
fn baruch_container_has_the_promised_layout() {
    let doc = sample_document();
    let bytes = write_baruch(&doc).unwrap();
    // Wie bei EPUB/ODF: Der erste lokale Kopf heißt „mimetype“, ohne Zusatzfeld, und der Inhalt folgt
    // unkomprimiert ab Byte 38 – so erkennt man den Typ ohne Entpacken.
    assert_eq!(&bytes[0..4], b"PK\x03\x04", "ZIP-Signatur");
    assert_eq!(u16::from_le_bytes([bytes[26], bytes[27]]), 8, "Namenslänge von mimetype");
    assert_eq!(u16::from_le_bytes([bytes[28], bytes[29]]), 0, "kein Zusatzfeld am ersten Eintrag");
    assert_eq!(&bytes[30..38], b"mimetype", "erster Eintrag heißt mimetype");
    assert_eq!(&bytes[38..38 + ZIP_MIMETYPE.len()], ZIP_MIMETYPE.as_bytes(), "Inhalt unkomprimiert direkt dahinter");

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names = Vec::new();
    let mut contents = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        names.push(entry.name().unwrap().into_owned());
        contents.push((entry.name().unwrap().into_owned(), entry.compression(), data));
    }
    // Reihenfolge: mimetype, werk.xml, Bilder in der Reihenfolge des Dokuments, MANIFEST.xml zuletzt; sonst nichts.
    let mut expected = vec![ENTRY_MIMETYPE.to_owned(), ENTRY_WORK.to_owned()];
    let revisions: Vec<&ExchangeAssetRevision> = doc.work.assets.iter().flat_map(|a| &a.revisions).collect();
    expected.extend(revisions.iter().map(|r| image_entry(&r.uuid, &r.mime).unwrap()));
    expected.push(ENTRY_MANIFEST.to_owned());
    assert_eq!(names, expected, "Einträge und Reihenfolge nach F1, kein zusätzlicher Eintrag");

    let (_, method, data) = &contents[0];
    assert_eq!(*method, zip::CompressionMethod::Stored, "mimetype STORED");
    assert_eq!(data.as_slice(), ZIP_MIMETYPE.as_bytes(), "mimetype genau, ohne Zeilenende");
    assert_eq!(contents[1].1, zip::CompressionMethod::Deflated, "werk.xml DEFLATE");
    assert_eq!(contents.last().unwrap().1, zip::CompressionMethod::Deflated, "MANIFEST.xml DEFLATE");
    for (revision, (name, method, data)) in revisions.iter().zip(&contents[2..contents.len() - 1]) {
        // Bilddateien byte-gleich mit data; Raster schon komprimiert → STORED, SVG ist Text → DEFLATE.
        assert_eq!(data, &revision.data, "{name}: byte-gleich");
        let want = if revision.mime == "image/svg+xml" {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        assert_eq!(*method, want, "{name}: Kompression nach F1");
    }

    // Manifest: jeder Eintrag außer mimetype und sich selbst, mit passender SHA-256 und Bytezahl.
    let manifest = String::from_utf8(contents.last().unwrap().2.clone()).unwrap();
    let listed: Vec<(String, String, String)> = parse(&manifest)
        .into_iter()
        .filter(|e| e.path == "manifest/datei")
        .map(|e| (e.attr("pfad").unwrap().into(), e.attr("sha256").unwrap().into(), e.attr("bytes").unwrap().into()))
        .collect();
    let want: Vec<(String, String, String)> = contents[1..contents.len() - 1]
        .iter()
        .map(|(name, _, data)| (name.clone(), sha256_hex(data), data.len().to_string()))
        .collect();
    assert_eq!(listed, want, "Manifest nennt genau die Einträge mit Prüfsumme und Größe");

    // werk.xml im ZIP: jede Bildfassung verweist mit datei= auf ihren Eintrag und trägt kein <daten>.
    let work_xml = String::from_utf8(contents[1].2.clone()).unwrap();
    assert_eq!(work_xml, write_xml(&doc, ImageMode::Files).unwrap(), "werk.xml = write_xml(Files)");
    let elements = parse(&work_xml);
    for revision in &revisions {
        let at = index_of(&elements, "bild/fassung", &revision.uuid);
        assert_eq!(
            elements[at].attr("datei").map(str::to_owned),
            image_entry(&revision.uuid, &revision.mime),
            "datei= zeigt auf bilder/<fassungs-uuid>.<ext>"
        );
        assert!(children(&elements, at, "daten").is_empty(), "kein <daten> im ZIP");
    }
}

#[test]
fn root_carries_namespace_version_and_instance() {
    let mut doc = sample_document();
    // Geschrieben wird immer die eigene Unterversion, auch wenn das Dokument aus einer höheren gelesen wurde
    // (Unbekanntes daraus ist nicht im Modell und wird nicht mitgeschrieben).
    doc.version = FORMAT_VERSION + 1;
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<werk "), "Deklaration, dann <werk>");
    let elements = parse(&xml);
    let root = &elements[0];
    assert_eq!(root.attr("xmlns"), Some(NAMESPACE), "Namensraum urn:baruch:werk:1");
    assert_eq!(root.attr("version"), Some("9"), "Unterversion FORMAT_VERSION (9 seit Runde 34)");
    assert_eq!(FORMAT_VERSION, 9, "Runde 34 (Mindmap: titel, eltern, platz, hauptthema am Einfall) zählt auf 9");
    assert_eq!(root.attr("art"), Some("buch"), "Werkart");
    let output = elements.iter().find(|e| e.path == "werk/ausgabe").unwrap();
    assert_eq!(output.attr("instanzname"), Some("Probeinstanz"), "Instanzname");
    assert_eq!(output.attr("instanz").map(str::to_owned), Some(sample_uuid(2)), "Instanz-UUID");
    // Leere optionale Teile entfallen, Pflichtteile bleiben: ein Werk ohne Einfälle, Fäden, Bilder, Grabsteine.
    let mut bare = sample_document();
    bare.work.ideas.clear();
    bare.work.threads.clear();
    bare.work.assets.clear();
    bare.work.deleted.clear();
    bare.work.map = ExchangeMap::default();
    bare.work.book = None;
    let xml = write_xml(&bare, ImageMode::Inline).unwrap();
    for absent in ["<einfaelle", "<faeden", "<bilder", "<geloescht", "<buch", "<karte"] {
        assert!(!xml.contains(absent), "{absent} entfällt ohne Inhalt");
    }
    for present in ["<vorlage>", "<gliederung>", "<angaben>"] {
        assert!(xml.contains(present), "{present} steht immer da");
    }
    // Ohne Bilder besteht das ZIP aus mimetype, werk.xml und MANIFEST.xml.
    let archive = zip::ZipArchive::new(std::io::Cursor::new(write_baruch(&bare).unwrap())).unwrap();
    let names: Vec<String> = archive.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert_eq!(names.len(), 3, "drei Einträge: {names:?}");
}
