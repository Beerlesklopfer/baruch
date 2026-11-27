// SPDX-License-Identifier: AGPL-3.0-or-later
//! Security of the reader (Round 32, docx_import): limits before and during unpacking, paths, relationship
//! targets, DTD and entities, foreign formats. No test needs the network or files outside memory.

#[path = "data/import/mod.rs"]
mod data;

use baruch_docx::read::read;
use baruch_docx::resolve::{View, resolve};
use baruch_docx::to_typst::to_typst;
use baruch_docx::{DocxError, Limits};
use data::{p, r};

fn small() -> Vec<(&'static str, Vec<u8>)> {
    data::parts(&format!("{}{}", p("Heading1", &r("Eins")), p("", &r("Text."))), "")
}

fn limit(result: Result<baruch_docx::read::ReadDocx, DocxError>) -> String {
    match result {
        Err(DocxError::Limit(message)) => message,
        other => panic!("Limit erwartet, bekommen: {other:?}"),
    }
}

#[test]
fn limits_are_checked_before_and_while_unpacking() {
    let bytes = data::zip(&small());
    let too_small = Limits { max_file_bytes: 100, ..Limits::DEFAULT };
    assert!(limit(read(&bytes, &too_small)).contains("Datei größer"), "die gepackte Datei selbst ist begrenzt");

    let few = Limits { max_entries: 3, ..Limits::DEFAULT };
    assert!(limit(read(&bytes, &few)).contains("Einträge"), "zu viele Einträge: abgelehnt, bevor einer entpackt ist");

    // A part that packs from 200 000 zeros down to a few hundred bytes (a decompression bomb in miniature).
    let mut bomb = small();
    bomb.push(("word/media/nullen.bin", vec![0u8; 200_000]));
    let bomb = data::zip(&bomb);
    assert!(bomb.len() < 20_000, "die Bombe ist gepackt klein ({} Bytes)", bomb.len());
    let entry = Limits { max_entry_bytes: 100_000, ..Limits::DEFAULT };
    assert!(
        limit(read(&bomb, &entry)).contains("nullen.bin"),
        "ein Eintrag über der Grenze je Eintrag wird genannt und abgelehnt"
    );
    let total = Limits { max_unpacked_bytes: 150_000, ..Limits::DEFAULT };
    assert!(limit(read(&bomb, &total)).contains("entpackt größer"), "die Summe aller Einträge ist begrenzt");

    let mut big_xml = small();
    let long = p("", &r(&"Wort ".repeat(4000)));
    big_xml[2] = ("word/document.xml", data::document(&long).into_bytes());
    let big_xml = data::zip(&big_xml);
    let xml = Limits { max_xml_bytes: 10_000, ..Limits::DEFAULT };
    assert!(limit(read(&big_xml, &xml)).contains("word/document.xml"), "ein XML-Teil über der XML-Grenze");
    let text = Limits { max_text_bytes: 1000, ..Limits::DEFAULT };
    assert!(limit(read(&big_xml, &text)).contains("Text"), "der Text des Dokuments ist begrenzt");

    let elements = Limits { max_elements: 5, ..Limits::DEFAULT };
    assert!(limit(read(&bytes, &elements)).contains("Elemente"), "die Zahl der XML-Elemente je Teil ist begrenzt");
    let mut deep = small();
    let nested = format!("{}{}{}", "<w:customXml>".repeat(40), p("", &r("tief")), "</w:customXml>".repeat(40));
    deep[2] = ("word/document.xml", data::document(&nested).into_bytes());
    let depth = Limits { max_depth: 20, ..Limits::DEFAULT };
    assert!(limit(read(&data::zip(&deep), &depth)).contains("Ebenen"), "die Tiefe der Verschachtelung ist begrenzt");

    let mut images = String::new();
    let mut rels = String::new();
    for n in 0..3 {
        images.push_str(&p("", &data::drawing(&format!("rId3{n}"), "")));
        rels.push_str(&format!(
            "<Relationship Id=\"rId3{n}\" \
             Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" \
             Target=\"media/bild{n}.png\"/>"
        ));
    }
    let mut many = data::parts(&format!("{}{images}", p("Heading1", &r("Bilder"))), &rels);
    many.push(("word/media/bild0.png", data::PNG.to_vec()));
    many.push(("word/media/bild1.png", data::PNG.to_vec()));
    many.push(("word/media/bild2.png", data::PNG.to_vec()));
    let two = Limits { max_images: 2, ..Limits::DEFAULT };
    assert!(limit(read(&data::zip(&many), &two)).contains("Bilder"), "die Zahl der Bilder ist begrenzt");
}

/// Forge the unpacked size in the header (local and in the central directory) to `claimed`.
fn forge_size(archive: &mut [u8], name: &str, claimed: u32) {
    let name = name.as_bytes();
    let mut patched = 0;
    for at in 0..archive.len().saturating_sub(46 + name.len()) {
        let (offset, name_at) = match &archive[at..at + 4] {
            b"PK\x03\x04" => (22, 30),
            b"PK\x01\x02" => (24, 46),
            _ => continue,
        };
        if &archive[at + name_at..at + name_at + name.len()] == name {
            archive[at + offset..at + offset + 4].copy_from_slice(&claimed.to_le_bytes());
            patched += 1;
        }
    }
    assert_eq!(patched, 2, "lokaler Kopf und Zentralverzeichnis gefälscht");
}

#[test]
fn a_forged_size_in_the_header_is_not_trusted() {
    let mut parts = small();
    let long = p("", &r(&"x".repeat(100_000)));
    parts[2] = ("word/document.xml", data::document(&long).into_bytes());
    let mut archive = data::zip(&parts);
    forge_size(&mut archive, "word/document.xml", 10);
    let limits = Limits { max_xml_bytes: 5_000, ..Limits::DEFAULT };
    let result = read(&archive, &limits);
    assert!(
        matches!(result, Err(DocxError::Limit(_) | DocxError::Archive(_))),
        "der Kopf behauptet 10 Bytes, entpackt sind es über 100 000: Der Leser hört an der Grenze auf (Limit) oder \
         das Archiv fällt als widersprüchlich auf (Archive) – nie wird der Teil ganz entpackt: {result:?}"
    );
}

#[test]
fn paths_and_relationship_targets_stay_inside_the_archive() {
    let mut parts = small();
    parts.push(("word/../../draussen.xml", b"<x/>".to_vec()));
    assert!(
        matches!(read(&data::zip(&parts), &Limits::DEFAULT), Err(DocxError::Unsafe(m)) if m.contains("draussen.xml")),
        "ein Eintrag mit .. im Namen ist unsicher – die ganze Datei wird abgelehnt"
    );
    for target in ["../../etc/passwd", "file:///etc/passwd", "https://example.org/bild.png", "C:\\bild.png"] {
        let rel = format!(
            "<Relationship Id=\"rId30\" \
             Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{target}\"/>"
        );
        let body = format!("{}{}", p("Heading1", &r("Eins")), p("", &data::drawing("rId30", "")));
        let result = read(&data::zip(&data::parts(&body, &rel)), &Limits::DEFAULT);
        assert!(
            matches!(&result, Err(DocxError::Unsafe(m)) if m.contains(target)),
            "ein Beziehungsziel außerhalb der Datei ({target}) ohne TargetMode External ist unsicher: {result:?}"
        );
    }
}

#[test]
fn linked_images_are_never_fetched() {
    let rel = "<Relationship Id=\"rId30\" \
        Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" \
        Target=\"http://127.0.0.1:9/nie-abgerufen.png\" TargetMode=\"External\"/>";
    let body = format!(
        "{}<w:p>{}<w:r><w:drawing><wp:inline><wp:docPr id=\"1\" name=\"x\"/><a:graphic><a:graphicData><pic:pic>\
         <pic:blipFill><a:blip r:link=\"rId30\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline>\
         </w:drawing></w:r></w:p>{}",
        p("Heading1", &r("Eins")),
        r("Mit verknüpftem Bild."),
        p("", &data::drawing("rId30", "eingebettet behauptet, aber extern")),
    );
    let read = read(&data::zip(&data::parts(&body, rel)), &Limits::DEFAULT)
        .expect("eine Verknüpfung nach außen macht die Datei nicht unlesbar");
    assert!(read.document.images.is_empty(), "ein verknüpftes Bild wird nie abgerufen und kommt nicht ins Werk");
    let warning = read.warnings.iter().find(|w| w.code == "docx_bild_extern").expect("gemeldet mit festem Code");
    assert!(warning.message.contains("(2 Stellen)"), "beide Formen (r:link und externes r:embed): {}", warning.message);
    assert_eq!(
        to_typst(&read.document).sections[0].body,
        "Mit verknüpftem Bild.",
        "der Text bleibt, ein Verweis auf das fremde Bild entsteht nicht"
    );
}

#[test]
fn dtd_and_entities_are_never_evaluated() {
    let mut parts = small();
    let bomb = "<?xml version=\"1.0\"?><!DOCTYPE w:document [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;\">\
        <!ENTITY x SYSTEM \"file:///etc/passwd\">]>";
    let doc = data::document(&p("", &r("&b; &x;")))
        .replace("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>", bomb);
    parts[2] = ("word/document.xml", doc.into_bytes());
    let result = read(&data::zip(&parts), &Limits::DEFAULT);
    assert!(
        matches!(&result, Err(DocxError::Unsafe(m)) if m.contains("DTD")),
        "ein Teil mit DTD wird abgelehnt, nichts daraus ausgewertet: {result:?}"
    );

    let mut parts = small();
    parts[2] = (
        "word/document.xml",
        data::document(&format!("{}{}", p("Heading1", &r("Eins")), p("", &r("A &amp; B &#x43; &fremd; &lt;"))))
            .into_bytes(),
    );
    let read = read(&data::zip(&parts), &Limits::DEFAULT).expect("ohne DTD lesbar");
    assert_eq!(
        to_typst(&resolve(&read.document, View::Original)).sections[0].body,
        "A & B C &fremd; \\<",
        "nur die festen Entitäten und Zeichenverweise werden aufgelöst; eine unbekannte bleibt wörtlich Text"
    );
}

#[test]
fn other_formats_are_refused_in_plain_words() {
    let not = |bytes: &[u8]| match read(bytes, &Limits::DEFAULT) {
        Err(DocxError::NotDocx(message)) => message,
        other => panic!("NotDocx erwartet, bekommen: {other:?}"),
    };
    let mut ole = vec![0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
    ole.extend_from_slice(&[0; 512]);
    let message = not(&ole);
    assert!(
        message.contains(".doc") && message.contains("Kennwort"),
        "eine OLE-Datei ist ein altes .doc oder verschlüsselt – beides steht im Klartext da: {message}"
    );
    assert!(not(b"Nur ein Text, kein Archiv.").contains("kein ZIP"), "eine Textdatei ist kein ZIP");
    assert!(not(b"").contains("kein ZIP"), "eine leere Datei auch nicht");
    let other_zip = data::zip(&[("mimetype", b"application/epub+zip".to_vec())]);
    assert!(not(&other_zip).contains("[Content_Types].xml"), "ein anderes ZIP (hier ein EPUB) ist keine Office-Datei");
    let sheet = data::zip(&[
        ("[Content_Types].xml", data::CONTENT_TYPES.as_bytes().to_vec()),
        ("xl/workbook.xml", b"<workbook/>".to_vec()),
    ]);
    assert!(
        not(&sheet).contains("kein Word-Dokument"),
        "eine Office-Datei ohne word/document.xml ist kein Word-Dokument"
    );
    let mut parts = small();
    parts[2] = ("word/document.xml", b"<w:document><w:body><w:p>".to_vec());
    assert!(
        matches!(read(&data::zip(&parts), &Limits::DEFAULT), Err(DocxError::Xml { part, .. }) if part == "word/document.xml"),
        "kaputtes XML nennt den Teil"
    );
}
