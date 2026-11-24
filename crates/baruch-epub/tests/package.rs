// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 20a (epub): EPUB als ZIP – Reihenfolge und Form nach OCF, Manifest/Spine, Navigation, Wohlgeformtheit
//! (xmllint `--noout --nonet`, sonst quick-xml mit Ausgabe „ÜBERSPRUNGEN“), gleiche Bytes bei gleicher Eingabe.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use baruch_epub::{EpubBook, FootnoteMode, build, package, sample};

fn entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("gültiges ZIP");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (file.name().unwrap().into_owned(), bytes)
        })
        .collect()
}

fn book_and_zip(mode: FootnoteMode) -> (EpubBook, Vec<u8>) {
    let mut input = sample::input();
    input.footnotes = mode;
    let book = build(&input).unwrap();
    let zip = package(&book).unwrap();
    (book, zip)
}

/// Werte eines Attributs in allen Vorkommen von `<tag …>`.
fn attrs(xml: &str, tag: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in xml.match_indices(&format!("<{tag} ")) {
        let end = at + xml[at..].find('>').unwrap();
        let element = &xml[at..end];
        if let Some(pos) = element.find(&format!(" {attr}=\"")) {
            let rest = &element[pos + attr.len() + 3..];
            out.push(rest[..rest.find('"').unwrap()].to_owned());
        }
    }
    out
}

#[test]
fn mimetype_comes_first_stored_and_without_extra_field() {
    let (_, zip) = book_and_zip(FootnoteMode::Popup);
    assert_eq!(&zip[0..4], b"PK\x03\x04", "lokaler Dateikopf am Anfang");
    assert_eq!(u16::from_le_bytes([zip[8], zip[9]]), 0, "Kompression 0 = STORED (OCF verlangt das)");
    let name_len = usize::from(u16::from_le_bytes([zip[26], zip[27]]));
    assert_eq!(u16::from_le_bytes([zip[28], zip[29]]), 0, "kein Extra-Feld, damit mimetype bei Byte 38 steht");
    assert_eq!(&zip[30..30 + name_len], b"mimetype", "erster Eintrag heißt mimetype");
    assert_eq!(&zip[38..38 + 20], b"application/epub+zip", "Inhalt unkomprimiert an fester Stelle");
}

#[test]
fn container_points_to_the_opf_and_the_manifest_lists_every_file_once() {
    let (book, zip) = book_and_zip(FootnoteMode::Popup);
    let files = entries(&zip);
    assert_eq!(files[1].0, "META-INF/container.xml", "container.xml als zweiter Eintrag");
    let container = String::from_utf8(files[1].1.clone()).unwrap();
    assert!(container.contains("full-path=\"OEBPS/content.opf\""), "verweist auf das OPF: {container}");
    let opf = String::from_utf8(files.iter().find(|f| f.0 == "OEBPS/content.opf").unwrap().1.clone()).unwrap();
    let manifest_hrefs = attrs(&opf, "item", "href");
    let unique: BTreeSet<&String> = manifest_hrefs.iter().collect();
    assert_eq!(unique.len(), manifest_hrefs.len(), "jede Datei genau einmal im Manifest: {manifest_hrefs:?}");
    let in_zip: BTreeSet<String> = files
        .iter()
        .filter_map(|f| f.0.strip_prefix("OEBPS/"))
        .filter(|f| *f != "content.opf")
        .map(str::to_owned)
        .collect();
    let in_manifest: BTreeSet<String> = manifest_hrefs.iter().cloned().collect();
    assert_eq!(in_zip, in_manifest, "jede Datei im ZIP steht im Manifest und umgekehrt");
    let ids: BTreeSet<String> = attrs(&opf, "item", "id").into_iter().collect();
    for idref in attrs(&opf, "itemref", "idref") {
        assert!(ids.contains(&idref), "Spine nennt nur Manifest-ids: {idref}");
    }
    assert_eq!(attrs(&opf, "itemref", "idref"), book.spine, "Spine im OPF wie im Buch");
}

#[test]
fn navigation_ncx_and_cover_are_declared_for_epub_3_and_2() {
    let (book, _) = book_and_zip(FootnoteMode::Popup);
    let opf = &book.opf;
    assert!(
        opf.contains("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>"),
        "nav.xhtml mit properties=nav: {opf}"
    );
    assert!(opf.contains("<item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>"), "toc.ncx");
    assert!(opf.contains("<spine toc=\"ncx\">"), "spine toc=ncx für EPUB-2-Leser");
    assert!(
        opf.contains(
            "<item id=\"cover-image\" href=\"images/cover.png\" media-type=\"image/png\" properties=\"cover-image\"/>"
        ),
        "cover-image im Manifest"
    );
    assert!(opf.contains("<meta name=\"cover\" content=\"cover-image\"/>"), "meta name=cover");
}

#[test]
fn building_and_packing_twice_gives_the_same_bytes() {
    let (_, a) = book_and_zip(FootnoteMode::Popup);
    let (_, b) = book_and_zip(FootnoteMode::Popup);
    assert_eq!(a, b, "gleiche Eingabe → gleiche Bytes (feste Zeitstempel, feste Reihenfolge)");
    let (_, c) = book_and_zip(FootnoteMode::Endnotes);
    assert_ne!(a, c, "andere Fußnotenart → anderes EPUB");
}

/// Wohlgeformt nach xmllint (`--noout --nonet`), sonst nach quick-xml. Gibt zurück, ob xmllint geprüft hat.
fn well_formed(name: &str, bytes: &[u8], dir: &Path) -> bool {
    if Path::new("/usr/bin/xmllint").exists() {
        let file = dir.join(name.replace('/', "_"));
        std::fs::write(&file, bytes).unwrap();
        let out =
            std::process::Command::new("/usr/bin/xmllint").args(["--noout", "--nonet"]).arg(&file).output().unwrap();
        assert!(out.status.success(), "{name} nicht wohlgeformt: {}", String::from_utf8_lossy(&out.stderr));
        true
    } else {
        let mut reader = quick_xml::Reader::from_reader(bytes);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("{name} nicht wohlgeformt: {e}"),
            }
            buf.clear();
        }
        false
    }
}

#[test]
fn every_xml_file_in_the_package_is_well_formed() {
    let dir = tempfile::tempdir().unwrap();
    let mut xmllint = true;
    for mode in [FootnoteMode::Popup, FootnoteMode::Endnotes] {
        let (_, zip) = book_and_zip(mode);
        let mut checked = 0;
        for (name, bytes) in entries(&zip) {
            if name.ends_with(".xhtml") || name.ends_with(".ncx") || name.ends_with(".opf") || name.ends_with(".xml") {
                xmllint &= well_formed(&name, &bytes, dir.path());
                checked += 1;
            }
        }
        assert!(checked >= 10, "container, OPF, NCX, nav und alle Textdateien geprüft ({checked})");
    }
    if !xmllint {
        println!("ÜBERSPRUNGEN: xmllint fehlt, Wohlgeformtheit nur mit quick-xml geprüft");
    }
}

#[test]
fn a_path_leaving_oebps_is_refused_when_packing() {
    let (mut book, _) = book_and_zip(FootnoteMode::Popup);
    book.items[0].href = "../draussen.xhtml".into();
    assert!(matches!(package(&book), Err(baruch_epub::EpubError::Invalid(_))), "kein Pfad aus OEBPS hinaus");
}
