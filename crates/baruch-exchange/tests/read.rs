// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lesen und Prüfen einer Austauschdatei (Runde 26, Vertrag F2/F7). Handgeschriebene Prüfdateien
//! unter `tests/data/import/` (frei erfunden), Prüf-ZIPs entstehen im Test mit `zip`. Jede Zusicherung mit
//! Begründung.

use std::io::{Cursor, Write};
use std::path::Path;

use baruch_exchange::read::{read, read_baruch, read_manifest, read_xml, validate};
use baruch_exchange::sample::{sample_document, sample_uuid};
use baruch_exchange::write::{ImageMode, write_baruch, write_xml};
use baruch_exchange::{
    ExchangeAuthor, ExchangeError, ExchangeIdeaStatus, ExchangeIdeaUse, ExchangeLinkKind, ExchangeNodeKind,
    ExchangeRevision, ExchangeRevisionStatus, ExchangeTombstone, ExchangeTombstoneKind, FORMAT_VERSION, Limits,
    sha256_hex,
};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const WERK: &str = include_str!("data/import/werk.baruch.xml");
const LACHEN: &str = include_str!("data/import/lachen.baruch.xml");
const EXTERN: &str = include_str!("data/import/extern.baruch.xml");

const PNG_UUID: &str = "c0000000-0000-4000-8000-000000000061";
const SVG_UUID: &str = "c0000000-0000-4000-8000-000000000063";

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import").join(name)
}

fn kind(result: Result<baruch_exchange::ExchangeDocument, ExchangeError>) -> String {
    match result {
        Ok(_) => "Ok".into(),
        Err(e) => format!("{e:?}").split(['(', ' ', '{']).next().unwrap_or("").to_owned(),
    }
}

#[test]
fn the_handwritten_file_is_read_exactly() {
    let doc = read_xml(WERK.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.version, 1, "Unterversion 1");
    assert_eq!(doc.instance.name, "Prüfrechner", "Instanzname aus <ausgabe>");
    assert!(doc.unknown.is_empty(), "nichts Unbekanntes: {:?}", doc.unknown);
    let work = &doc.work;
    assert_eq!(work.title, "Die Mühle am Bach", "Titel");
    assert_eq!(work.numbering.as_deref(), Some("I.;1.;a)"), "eigenes Muster");
    assert_eq!(
        work.timing.as_ref().map(|t| (t.words_per_minute, t.target_seconds)),
        Some((140, Some(1200))),
        "Sprechdauer"
    );
    assert_eq!(work.sword_sources.len(), 2, "zwei Quellen");
    let book = work.book.as_ref().unwrap();
    assert_eq!(
        (book.paper_mm, book.bleed_mm, book.front_matter, book.cover_pages),
        (0.12, 3.0, true, false),
        "Buchangaben"
    );
    assert_eq!(book.blurb, "Wasser & Mehl.", "&amp; aufgelöst");
    assert_eq!(work.template.current, "#set page(paper: \"a5\")\n", "Vorlagentext mit Zeilenende, nicht getrimmt");
    assert_eq!(work.template.revisions[1].note, "A5 gesetzt.", "Notiz der Vorlagenfassung");

    let part = &work.outline[0];
    assert_eq!(part.kind, ExchangeNodeKind::Part, "Teil");
    let wheel = &part.children[0];
    assert!(!wheel.numbered, "nummeriert=false");
    assert_eq!(
        wheel.revisions[0].body, "  == Das Rad\n\n\tEs dreht <&> und endet nie mit ]]> – Grüße.\r\nÄpfel.  ",
        "Text exakt: Ränder, Tab, Entitäten, ]]> und &#13; als echtes CR"
    );
    assert_eq!(wheel.revisions[1].body, "Steuer\u{1}zeichen\n", "kodierung=base64 ergibt das verbotene Steuerzeichen");
    assert_eq!(
        wheel.revisions[1].author,
        ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "pruef/modell-3b".into() },
        "Autor mit Modell"
    );
    assert_eq!(wheel.revisions[1].status, ExchangeRevisionStatus::Proposed, "Status vorschlag");
    assert_eq!(wheel.revisions[1].based_on.as_deref(), Some("c0000000-0000-4000-8000-000000000111"), "basis");
    assert_eq!(wheel.posts[0].refs[0].quote, "Es dreht", "Bezug mit Zitat");
    assert_eq!(wheel.children[0].kind, ExchangeNodeKind::Section, "Abschnitt unter dem Kapitel");

    assert_eq!(work.ideas[0].words, vec!["Rad", "Wasser"], "Schlagwörter in Reihenfolge");
    assert_eq!(work.ideas[1].status, ExchangeIdeaStatus::Done, "erledigt");
    assert_eq!(work.ideas[1].done_as, Some(ExchangeIdeaUse::Post), "erledigt als Beitrag");
    assert_eq!(work.ideas[1].deleted.as_deref(), Some("2026-02-08T10:00:00.000Z"), "gelöscht mit Zeitpunkt");
    assert_eq!(work.threads[0].posts[1].sources, vec!["Notizbuch"], "Quelle des Fadenbeitrags");
    let png = &work.assets[0].revisions[0];
    assert_eq!(png.data.len(), 70, "Base64 mit Zeilenumbruch in <daten> entschlüsselt");
    assert_eq!(sha256_hex(&png.data), png.sha256, "Prüfsumme passt");
    assert_eq!(work.deleted.len(), 1, "ein Grabstein");
    // `read` erkennt XML am Anfang und liefert dasselbe.
    assert_eq!(read(WERK.as_bytes(), &Limits::DEFAULT).unwrap(), doc, "read = read_xml bei XML");
}

#[test]
fn the_handwritten_file_is_valid_against_the_xsd() {
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1.xsd");
    let output = std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(data("werk.baruch.xml"))
        .output();
    match output {
        Ok(output) => assert!(
            output.status.success(),
            "xmllint lehnt die Prüfdatei ab: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(e) => {
            eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e}); die Rust-Prüfung oben sichert die Pflichtelemente")
        }
    }
}

#[test]
fn dtds_and_entities_are_refused_as_unsafe() {
    // Billion Laughs: abgelehnt, bevor eine Entität aufgelöst wird (sonst Milliarden Bytes).
    assert!(matches!(read_xml(LACHEN.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Unsafe(_))), "Billion Laughs");
    // Externe Entität: kein Dateizugriff, Ablehnung schon an der DTD.
    assert!(matches!(read_xml(EXTERN.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Unsafe(_))), "externe Entität");
    // Unbekannte Entitätsreferenz ohne DTD.
    let unknown = WERK.replace("Wasser &amp; Mehl.", "Wasser &nbsp; Mehl.");
    assert!(
        matches!(read_xml(unknown.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Unsafe(_))),
        "unbekannte Entität"
    );
    // Verarbeitungsanweisung außer der Deklaration.
    let pi = WERK.replace("<werk xmlns", "<?xml-stylesheet href=\"boese.xsl\"?>\n<werk xmlns");
    assert!(
        matches!(read_xml(pi.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Unsafe(_))),
        "Verarbeitungsanweisung"
    );
}

#[test]
fn every_limit_is_enforced() {
    let limit = |limits: Limits| matches!(read_xml(WERK.as_bytes(), &limits), Err(ExchangeError::Limit(_)));
    // Die Prüfdatei ist sechs Ebenen tief (werk/gliederung/punkt/punkt/punkt/fassung/text = 7).
    assert!(limit(Limits { max_depth: 5, ..Limits::DEFAULT }), "Tiefe");
    assert!(limit(Limits { max_elements: 20, ..Limits::DEFAULT }), "Zahl der Elemente");
    assert!(limit(Limits { max_attributes: 4, ..Limits::DEFAULT }), "Attribute je Element (<fassung> hat 9)");
    assert!(limit(Limits { max_text_bytes: 40, ..Limits::DEFAULT }), "Bytes je Text");
    assert!(limit(Limits { max_xml_bytes: 1000, ..Limits::DEFAULT }), "Bytes des ganzen XML");
    assert!(
        matches!(
            read(WERK.as_bytes(), &Limits { max_file_bytes: 1000, ..Limits::DEFAULT }),
            Err(ExchangeError::Limit(_))
        ),
        "Bytes der ganzen Datei"
    );
    assert!(read_xml(WERK.as_bytes(), &Limits::DEFAULT).is_ok(), "mit den Vorgaben geht es");
}

#[test]
fn a_wrong_namespace_is_refused() {
    let other_major = WERK.replace("urn:baruch:werk:1", "urn:baruch:werk:2");
    assert!(
        matches!(read_xml(other_major.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Version { ref namespace, .. }) if namespace == "urn:baruch:werk:2"),
        "andere Hauptversion → Version"
    );
    let foreign = WERK.replace("urn:baruch:werk:1", "urn:anderes:format");
    assert_eq!(kind(read_xml(foreign.as_bytes(), &Limits::DEFAULT)), "NotExchange", "fremder Namensraum");
    let none = WERK.replace(" xmlns=\"urn:baruch:werk:1\"", "");
    assert_eq!(kind(read_xml(none.as_bytes(), &Limits::DEFAULT)), "NotExchange", "ohne Namensraum");
    assert_eq!(kind(read(b"Hallo Welt", &Limits::DEFAULT)), "NotExchange", "weder ZIP noch XML");
}

/// Seit Runde 25 ist die höhere Unterversion `FORMAT_VERSION + 1` (3): `<karte>` kennt der Leser dann schon, nur ein
/// unbekanntes Kind darin wird gemeldet.
#[test]
fn a_newer_minor_version_is_read_and_unknown_parts_are_listed_but_not_taken() {
    let original = read_xml(WERK.as_bytes(), &Limits::DEFAULT).unwrap();
    let newer_version = (FORMAT_VERSION + 1).to_string();
    let newer = WERK
        .replace(
            "version=\"1\" uuid=\"c0000000-0000-4000-8000-000000000001\"",
            &format!("version=\"{newer_version}\" uuid=\"c0000000-0000-4000-8000-000000000001\""),
        )
        .replace("<bibelmodul>", "<farbe>blau</farbe>\n    <bibelmodul>")
        .replace("art=\"teil\" nummeriert=\"true\">", "art=\"teil\" nummeriert=\"true\" stimmung=\"heiter\">")
        .replace(
            "</werk>",
            "  <karte farbe=\"grau\"><kante von=\"a\" nach=\"b\"/></karte>\n  <x:fremd xmlns:x=\"urn:ganz:anders\">?</x:fremd>\n</werk>",
        );
    let doc = read_xml(newer.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.version, FORMAT_VERSION + 1, "höhere Unterversion wird gelesen");
    for path in [
        "werk/angaben/farbe",
        "werk/gliederung/punkt/@stimmung",
        "werk/karte/kante",
        "werk/karte/@farbe",
        "werk/{urn:ganz:anders}fremd",
    ] {
        assert!(doc.unknown.iter().any(|u| u == path), "{path} als unbekannt gemeldet: {:?}", doc.unknown);
    }
    assert!(!doc.unknown.iter().any(|u| u == "werk/karte"), "<karte> selbst ist ab Unterversion 2 bekannt");
    assert_eq!(doc.work, original.work, "übernommen wird davon nichts: das Werk ist dasselbe");
}

#[test]
fn missing_required_parts_are_xml_errors() {
    let no_title = WERK.replace("<titel>Die Mühle am Bach</titel>", "");
    assert!(
        matches!(read_xml(no_title.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(ref m)) if m.contains("werk/angaben/titel")),
        "Pflichtelement fehlt, mit Pfad"
    );
    let no_uuid = WERK.replacen(" uuid=\"c0000000-0000-4000-8000-000000000001\"", "", 1);
    assert!(
        matches!(read_xml(no_uuid.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(_))),
        "Pflichtattribut fehlt"
    );
    let broken = WERK.replace("</gliederung>", "");
    assert_eq!(kind(read_xml(broken.as_bytes(), &Limits::DEFAULT)), "Xml", "nicht wohlgeformt");
}

#[test]
fn validate_checks_uuids_duplicates_references_and_the_current_mark() {
    assert!(validate(&sample_document()).is_ok(), "das Beispielwerk ist gültig");
    let invalid = |change: &dyn Fn(&mut baruch_exchange::ExchangeDocument)| {
        let mut doc = sample_document();
        change(&mut doc);
        matches!(validate(&doc), Err(ExchangeError::Invalid(_)))
    };
    assert!(
        invalid(&|d| d.work.outline[0].children[0].revisions[1].status = ExchangeRevisionStatus::Current),
        "zwei aktuelle"
    );
    assert!(invalid(&|d| d.work.outline[1].uuid = sample_uuid(10)), "Kennung doppelt");
    assert!(invalid(&|d| d.work.outline[0].uuid = d.work.outline[0].uuid.to_uppercase()), "Großbuchstaben");
    assert!(
        invalid(&|d| d.work.outline[0].children[0].revisions[1].based_on = Some(sample_uuid(121))),
        "basis in anderem Punkt"
    );
    assert!(invalid(&|d| d.work.ideas[0].node = Some(sample_uuid(999))), "Bezug auf unbekannten Punkt");
    assert!(invalid(&|d| d.work.book.as_mut().unwrap().cover = Some(sample_uuid(998))), "Umschlag ohne Bild");
    assert!(invalid(&|d| d.work.deleted[0].uuid = sample_uuid(11)), "Grabstein für ein vorhandenes Stück");
    assert!(invalid(&|d| d.work.outline[1].children[0].kind = ExchangeNodeKind::Part), "Teil unter einem Teil");
    let mut bad_sum = sample_document();
    bad_sum.work.assets[0].revisions[0].data.push(0);
    assert!(matches!(validate(&bad_sum), Err(ExchangeError::Checksum { .. })), "Prüfsumme der Bildbytes");
}

// ---- ZIP ---------------------------------------------------------------------------------------

/// werk.xml für den Container: die Bilder als `datei` statt `<daten>`.
fn zip_xml() -> String {
    let png_daten =
        WERK[WERK.find("<daten>iVBOR").unwrap()..].split_once("</daten>").unwrap().0.to_owned() + "</daten>";
    let svg_daten = WERK[WERK.find("<daten>PHN2").unwrap()..].split_once("</daten>").unwrap().0.to_owned() + "</daten>";
    WERK.replace(&png_daten, "")
        .replace(&svg_daten, "")
        .replace("status=\"aktuell\" autor=\"Lena Probe\" angelegt=\"2026-02-06T10:00:00.000Z\">", &format!("status=\"aktuell\" autor=\"Lena Probe\" angelegt=\"2026-02-06T10:00:00.000Z\" datei=\"bilder/{PNG_UUID}.png\">"))
        .replace("status=\"aktuell\" autor=\"Lena Probe\" angelegt=\"2026-02-06T11:00:00.000Z\">", &format!("status=\"aktuell\" autor=\"Lena Probe\" angelegt=\"2026-02-06T11:00:00.000Z\" datei=\"bilder/{SVG_UUID}.svg\">"))
}

struct Entry {
    name: String,
    bytes: Vec<u8>,
    method: CompressionMethod,
    listed: bool,
}

fn entry(name: &str, bytes: &[u8], method: CompressionMethod) -> Entry {
    Entry { name: name.into(), bytes: bytes.to_vec(), method, listed: true }
}

fn manifest(entries: &[Entry], extra: &[(&str, &str, u64)]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest xmlns=\"urn:baruch:werk:1\" version=\"1\">\n",
    );
    for e in entries.iter().filter(|e| e.listed && e.name != "mimetype" && e.name != "MANIFEST.xml") {
        out.push_str(&format!(
            "  <datei pfad=\"{}\" sha256=\"{}\" bytes=\"{}\"/>\n",
            e.name,
            sha256_hex(&e.bytes),
            e.bytes.len()
        ));
    }
    for (path, sha, bytes) in extra {
        out.push_str(&format!("  <datei pfad=\"{path}\" sha256=\"{sha}\" bytes=\"{bytes}\"/>\n"));
    }
    out.push_str("</manifest>\n");
    out
}

fn good_entries() -> Vec<Entry> {
    let doc = read_xml(WERK.as_bytes(), &Limits::DEFAULT).unwrap();
    vec![
        entry("mimetype", b"application/vnd.baruch.werk+zip", CompressionMethod::Stored),
        entry("werk.xml", zip_xml().as_bytes(), CompressionMethod::Deflated),
        entry(&format!("bilder/{PNG_UUID}.png"), &doc.work.assets[0].revisions[0].data, CompressionMethod::Stored),
        entry(&format!("bilder/{SVG_UUID}.svg"), &doc.work.assets[1].revisions[0].data, CompressionMethod::Deflated),
    ]
}

/// ZIP in der Reihenfolge der Einträge; das Manifest (falls `with_manifest`) zuletzt.
fn build(entries: &[Entry], extra_manifest: &[(&str, &str, u64)], with_manifest: bool) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for e in entries {
        zip.start_file(e.name.as_str(), SimpleFileOptions::default().compression_method(e.method)).unwrap();
        zip.write_all(&e.bytes).unwrap();
    }
    if with_manifest {
        zip.start_file("MANIFEST.xml", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))
            .unwrap();
        zip.write_all(manifest(entries, extra_manifest).as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Zentralverzeichnis durchgehen: (Stelle des Eintrags im Verzeichnis, Stelle des lokalen Kopfs, Name).
fn central(bytes: &[u8]) -> Vec<(usize, usize, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = bytes.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
    while bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let u16_at = |o: usize| usize::from(u16::from_le_bytes([bytes[at + o], bytes[at + o + 1]]));
        let local = u32::from_le_bytes([bytes[at + 42], bytes[at + 43], bytes[at + 44], bytes[at + 45]]) as usize;
        let (n, m, k) = (u16_at(28), u16_at(30), u16_at(32));
        out.push((at, local, bytes[at + 46..at + 46 + n].to_vec()));
        at += 46 + n + m + k;
    }
    out
}

/// Kopf eines Eintrags im lokalen Kopf und im Zentralverzeichnis ändern (Felder an gleicher Bedeutung).
fn patch(bytes: &mut [u8], name: &str, local_offset: usize, central_offset: usize, value: &[u8]) {
    for (at, local, entry) in central(bytes) {
        if entry == name.as_bytes() {
            bytes[at + central_offset..at + central_offset + value.len()].copy_from_slice(value);
            bytes[local + local_offset..local + local_offset + value.len()].copy_from_slice(value);
        }
    }
}

fn archive_error(bytes: &[u8], limits: &Limits) -> String {
    kind(read_baruch(bytes, limits))
}

#[test]
fn a_good_container_reads_like_the_xml_file() {
    let zip = build(&good_entries(), &[], true);
    let from_zip = read_baruch(&zip, &Limits::DEFAULT).unwrap();
    let from_xml = read_xml(WERK.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(from_zip.work, from_xml.work, "Bilder aus dem Archiv ergeben dasselbe Werk wie <daten>");
    assert_eq!(read(&zip, &Limits::DEFAULT).unwrap().work, from_xml.work, "read erkennt ZIP am Anfang");
    let manifest_xml = manifest(&good_entries(), &[]);
    assert_eq!(
        read_manifest(manifest_xml.as_bytes(), &Limits::DEFAULT).unwrap().len(),
        3,
        "Manifest nennt drei Einträge"
    );
}

#[test]
fn unsafe_names_are_refused() {
    for name in
        ["../werk2.xml", "/etc/passwd", "bilder\\boese.png", "C:boese.xml", "bilder/nicht-uuid.png", "anderes.txt"]
    {
        let mut entries = good_entries();
        entries.push(entry(name, b"x", CompressionMethod::Stored));
        assert_eq!(archive_error(&build(&entries, &[], true), &Limits::DEFAULT), "Archive", "{name} abgelehnt");
    }
}

#[test]
fn duplicates_unlisted_and_missing_entries_are_refused() {
    // Doppelter Name: zip schreibt keine Doppel, darum den Namen „werk.xmm“ nachträglich zu „werk.xml“ machen.
    let mut entries = good_entries();
    entries.push(entry("werk.xmm", b"<x/>", CompressionMethod::Stored));
    let mut zip = build(&entries, &[], true);
    let positions: Vec<usize> = zip.windows(8).enumerate().filter(|(_, w)| *w == b"werk.xmm").map(|(i, _)| i).collect();
    for at in positions {
        zip[at..at + 8].copy_from_slice(b"werk.xml");
    }
    assert!(
        matches!(read_baruch(&zip, &Limits::DEFAULT), Err(ExchangeError::Archive(ref m)) if m.contains("doppelt")),
        "Doppel erkannt"
    );
    // Eintrag, der nicht im Manifest steht.
    let mut entries = good_entries();
    entries[3].listed = false;
    assert!(
        matches!(read_baruch(&build(&entries, &[], true), &Limits::DEFAULT), Err(ExchangeError::Archive(ref m)) if m.contains("nicht im Manifest")),
        "nicht gelisteter Eintrag"
    );
    // Gelistete Datei fehlt.
    let missing = [("bilder/c0000000-0000-4000-8000-0000000000ff.png", &*sha256_hex(b"x"), 1u64)];
    assert_eq!(
        archive_error(&build(&good_entries(), &missing, true), &Limits::DEFAULT),
        "Archive",
        "gelistet, aber fehlt"
    );
    // Ohne Manifest.
    assert_eq!(archive_error(&build(&good_entries(), &[], false), &Limits::DEFAULT), "Archive", "Manifest fehlt");
}

#[test]
fn a_wrong_checksum_is_refused() {
    let mut entries = good_entries();
    let honest = manifest(&entries, &[]);
    entries[1].bytes = zip_xml().replace("Die Mühle am Bach", "Die Mühle am Fluss").into_bytes();
    // Manifest von vorher: werk.xml passt nicht mehr dazu.
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for e in &entries {
        zip.start_file(e.name.as_str(), SimpleFileOptions::default().compression_method(e.method)).unwrap();
        zip.write_all(&e.bytes).unwrap();
    }
    zip.start_file("MANIFEST.xml", SimpleFileOptions::default()).unwrap();
    zip.write_all(honest.as_bytes()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(
        matches!(read_baruch(&bytes, &Limits::DEFAULT), Err(ExchangeError::Checksum { ref path }) if path == "werk.xml"),
        "falsche Prüfsumme nennt den Eintrag"
    );
}

#[test]
fn the_mimetype_must_come_first_stored_and_exact() {
    let mut wrong = good_entries();
    wrong[0].bytes = b"application/zip".to_vec();
    assert_eq!(archive_error(&build(&wrong, &[], true), &Limits::DEFAULT), "Archive", "falscher Inhalt");
    let mut late = good_entries();
    late.swap(0, 1);
    assert_eq!(archive_error(&build(&late, &[], true), &Limits::DEFAULT), "Archive", "nicht zuerst");
    let mut packed = good_entries();
    packed[0].method = CompressionMethod::Deflated;
    assert_eq!(archive_error(&build(&packed, &[], true), &Limits::DEFAULT), "Archive", "komprimiert");
}

#[test]
fn encrypted_unsupported_and_linked_entries_are_refused() {
    // Verschlüsselt: Bit 0 der Flags (lokal +6, zentral +8).
    let mut zip = build(&good_entries(), &[], true);
    patch(&mut zip, "werk.xml", 6, 8, &[0x01, 0x00]);
    assert!(
        matches!(read_baruch(&zip, &Limits::DEFAULT), Err(ExchangeError::Archive(ref m)) if m.contains("verschlüsselt")),
        "verschlüsselter Eintrag"
    );
    // Kompression 12 (bzip2; lokal +8, zentral +10).
    let mut zip = build(&good_entries(), &[], true);
    patch(&mut zip, "werk.xml", 8, 10, &[12, 0]);
    assert!(
        matches!(read_baruch(&zip, &Limits::DEFAULT), Err(ExchangeError::Archive(ref m)) if m.contains("Kompression")),
        "nicht unterstützte Kompression"
    );
    // Verknüpfung mit erlaubtem Namen.
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for e in good_entries() {
        writer.start_file(e.name.as_str(), SimpleFileOptions::default().compression_method(e.method)).unwrap();
        writer.write_all(&e.bytes).unwrap();
    }
    writer
        .add_symlink("bilder/c0000000-0000-4000-8000-0000000000aa.png", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert!(
        matches!(read_baruch(&bytes, &Limits::DEFAULT), Err(ExchangeError::Archive(ref m)) if m.contains("Verknüpfung")),
        "Verknüpfung"
    );
}

#[test]
fn zip_bombs_and_lying_headers_hit_the_limits() {
    // Ein Eintrag, entpackt über der Grenze (stark komprimierbar: 4 MiB Nullen).
    let mut entries = good_entries();
    let zeros = vec![0u8; 4 * 1024 * 1024];
    entries.push(entry("bilder/c0000000-0000-4000-8000-0000000000bb.png", &zeros, CompressionMethod::Deflated));
    let bomb = build(&entries, &[], true);
    assert!(bomb.len() < 64 * 1024, "die Bombe ist klein gepackt ({} Bytes)", bomb.len());
    let limits = Limits { max_entry_bytes: 1024 * 1024, ..Limits::DEFAULT };
    assert_eq!(archive_error(&bomb, &limits), "Limit", "Eintrag über der Grenze");
    // Summe über der Grenze: jeder Eintrag darunter, zusammen darüber.
    let limits = Limits { max_unpacked_bytes: 2 * 1024 * 1024, ..Limits::DEFAULT };
    assert_eq!(archive_error(&bomb, &limits), "Limit", "Summe über der Grenze");
    // Gelogene Größe im Kopf: 4 MiB Nullen, im Kopf 100 Bytes (lokal +22, zentral +24). Den Kopfgrößen wird
    // nicht getraut: entpackt wird mit take(Grenze + 1).
    let mut lying = bomb.clone();
    patch(&mut lying, "bilder/c0000000-0000-4000-8000-0000000000bb.png", 22, 24, &100u32.to_le_bytes());
    let limits = Limits { max_entry_bytes: 1024 * 1024, ..Limits::DEFAULT };
    let result = read_baruch(&lying, &limits);
    assert!(
        matches!(result, Err(ExchangeError::Limit(_) | ExchangeError::Archive(_) | ExchangeError::Checksum { .. })),
        "gelogene Kopfgröße wird abgelehnt: {result:?}"
    );
    // Zu viele Einträge.
    assert_eq!(
        archive_error(&build(&good_entries(), &[], true), &Limits { max_entries: 3, ..Limits::DEFAULT }),
        "Limit",
        "Zahl der Einträge"
    );
}

/// Nachbesserung R26, umgestellt in Runde 25: Eine neuere Unterversion darf neue Arten von Grabsteinen schreiben.
/// Ein heutiger Leser meldet eine ihm unbekannte Art (hier die erfundene `wolke`) in `FORMAT_VERSION + 1` als
/// unbekannt und überspringt nur diesen Grabstein (§6); in der eigenen Unterversion bleibt ein unbekannter Wert ein
/// Fehler. Die Arten der Karte (`verbindung`, `lage`) gibt es erst ab Unterversion 2: in `version="1"` sind sie ein
/// Fehler, in 2 werden sie übernommen.
#[test]
fn an_unknown_tombstone_kind_is_skipped_in_a_newer_minor_version_but_refused_in_version_one() {
    let entry =
        "<eintrag uuid=\"c0000000-0000-4000-8000-000000000070\" art=\"punkt\" zeit=\"2026-02-09T10:00:00.000Z\"/>";
    assert!(WERK.contains(entry), "Vorbedingung: die Prüfdatei hat diesen Grabstein");
    let root_v1 = "version=\"1\" uuid=\"c0000000-0000-4000-8000-000000000001\"";
    let root = |v: u32| format!("version=\"{v}\" uuid=\"c0000000-0000-4000-8000-000000000001\"");
    let with_kind = |kind: &str| {
        WERK.replace(
            entry,
            &format!(
                "{entry}\n    <eintrag uuid=\"c0000000-0000-4000-8000-000000000071\" art=\"{kind}\" zeit=\"2026-02-10T10:00:00.000Z\"/>"
            ),
        )
    };

    // Erfundene Art: in der eigenen Unterversion ein Fehler, in einer höheren gemeldet und übersprungen.
    let own = with_kind("wolke").replace(root_v1, &root(FORMAT_VERSION));
    assert!(
        matches!(read_xml(own.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(ref m)) if m.contains("wolke")),
        "Unterversion {FORMAT_VERSION} kennt alle ihre Arten: ein fremder Wert ist ein Fehler"
    );
    let newer = with_kind("wolke").replace(root_v1, &root(FORMAT_VERSION + 1));
    let doc = read_xml(newer.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.work.deleted.len(), 1, "nur der bekannte Grabstein wird übernommen: {:?}", doc.work.deleted);
    assert!(
        doc.unknown.iter().any(|u| u.contains("geloescht/eintrag/@art") && u.contains("wolke")),
        "die neue Art wird als unbekannt gemeldet: {:?}",
        doc.unknown
    );

    // `verbindung` und `lage`: in Unterversion 1 ein Fehler (Vertrag §4.6), ab 2 übernommen.
    for (kind, expected) in [("verbindung", ExchangeTombstoneKind::Link), ("lage", ExchangeTombstoneKind::Position)] {
        let v1 = with_kind(kind);
        assert!(
            matches!(read_xml(v1.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(ref m)) if m.contains(kind) && m.contains("Unterversion 2")),
            "{kind} in Unterversion 1 ist ein Fehler mit Hinweis auf die Unterversion"
        );
        let v2 = v1.replace(root_v1, &root(2));
        let doc = read_xml(v2.as_bytes(), &Limits::DEFAULT).unwrap();
        assert!(
            doc.work.deleted.iter().any(|t| t.kind == expected && t.uuid.ends_with("000000000071")),
            "{kind} in Unterversion 2 wird ein Grabstein {expected:?}: {:?}",
            doc.work.deleted
        );
    }
}

/// Nachbesserung R26: Der Instanzname aus einer fremden Datei wird gesäubert, denn er landet im Git-Autor (der `<`,
/// `>` und Zeilenumbrüche ablehnt), in Antragsherkunft und Fassungsnotizen; er wird auf die lokale Höchstlänge gekürzt.
#[test]
fn the_instance_name_from_a_file_is_cleaned_and_shortened() {
    let long = "y".repeat(300);
    let evil = WERK
        .replace("instanzname=\"Prüfrechner\"", &format!("instanzname=\"  Prüf &lt;rechner&gt;&#10;&#9;zwei {long}\""));
    let doc = read_xml(evil.as_bytes(), &Limits::DEFAULT).unwrap();
    let name = &doc.instance.name;
    assert!(name.starts_with("Prüf ‹rechner› zwei y"), "Klammern ersetzt, Umbruch und Tab ein Leerzeichen: {name}");
    assert!(!name.contains(['<', '>', '\n', '\t']), "nichts, was Git im Autor ablehnt: {name}");
    assert_eq!(name.chars().count(), baruch_exchange::MAX_INSTANCE_NAME_CHARS, "gekürzt auf die Höchstlänge");
    assert_eq!(baruch_exchange::clean_instance_name("Prüfrechner"), "Prüfrechner", "ein sauberer Name bleibt");
}

// ---- Karte (Runde 25, Unterversion 2) -------------------------------------------------------------

/// Ein `<karte>`-Block zum Einsetzen in die Prüfdatei (Einfälle …40 und …41 gibt es dort).
const MAP: &str = "  <karte>\n    <verbindung uuid=\"c0000000-0000-4000-8000-000000000090\" von=\"c0000000-0000-4000-8000-000000000040\" nach=\"c0000000-0000-4000-8000-000000000041\" art=\"widerspricht\" herkunft-art=\"mcp\" herkunft=\"@mcp:probe\" angelegt=\"2026-02-11T10:00:00.000Z\" geaendert=\"2026-02-11T10:00:00.000Z\">\n      <notiz>Zwei Zeilen\nund &lt;Klammern&gt;</notiz>\n    </verbindung>\n    <lage uuid=\"c0000000-0000-4000-8000-000000000091\" einfall=\"c0000000-0000-4000-8000-000000000041\" x=\"-12.5\" y=\"1e3\" geaendert=\"2026-02-11T11:00:00.000Z\"/>\n  </karte>\n</werk>";

fn with_map(version: u32, map: &str) -> String {
    WERK.replace(
        "version=\"1\" uuid=\"c0000000-0000-4000-8000-000000000001\"",
        &format!("version=\"{version}\" uuid=\"c0000000-0000-4000-8000-000000000001\""),
    )
    .replace("</werk>", map)
}

#[test]
fn the_map_is_read_from_version_two_and_ignored_as_unknown_in_version_one() {
    let ideas: Vec<String> =
        read_xml(WERK.as_bytes(), &Limits::DEFAULT).unwrap().work.ideas.into_iter().map(|i| i.uuid).collect();
    assert!(
        ["c0000000-0000-4000-8000-000000000040", "c0000000-0000-4000-8000-000000000041"]
            .iter()
            .all(|u| ideas.iter().any(|i| i == u)),
        "Vorbedingung: beide Einfälle stehen in der Prüfdatei: {ideas:?}"
    );
    let doc = read_xml(with_map(2, MAP).as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "in Unterversion 2 ist die Karte bekannt: {:?}", doc.unknown);
    let link = &doc.work.map.links[0];
    assert_eq!(link.kind, ExchangeLinkKind::Contradicts, "Art widerspricht");
    assert_eq!(link.note, "Zwei Zeilen\nund <Klammern>", "Notiz exakt, Zeilenumbruch und Entitäten aufgelöst");
    assert_eq!((link.origin_name.as_str(), link.deleted.as_deref()), ("@mcp:probe", None), "Herkunft, nicht gelöscht");
    let position = &doc.work.map.positions[0];
    assert_eq!((position.x, position.y), (-12.5, 1000.0), "Koordinaten (auch in Exponentschreibweise)");

    let v1 = read_xml(with_map(1, MAP).as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(v1.unknown.iter().any(|u| u == "werk/karte"), "Unterversion 1: <karte> unbekannt: {:?}", v1.unknown);
    assert!(v1.work.map.is_empty(), "Unterversion 1: nichts aus der Karte übernommen");
}

#[test]
fn coordinates_must_be_finite_and_within_a_million() {
    for (raw, finite) in [("NaN", false), ("INF", false), ("1000000.0001", true), ("-1e7", true)] {
        let map = MAP.replace("x=\"-12.5\"", &format!("x=\"{raw}\""));
        let result = read_xml(with_map(2, &map).as_bytes(), &Limits::DEFAULT);
        if finite {
            assert!(
                matches!(result, Err(ExchangeError::Invalid(ref m)) if m.contains("Koordinate")),
                "{raw}: endlich, aber außerhalb ±1e6 → Invalid: {result:?}"
            );
        } else {
            assert!(matches!(result, Err(ExchangeError::Xml(_))), "{raw}: keine endliche Zahl → Xml: {result:?}");
        }
    }
    let edge = MAP.replace("x=\"-12.5\"", "x=\"-1000000\"");
    assert!(read_xml(with_map(2, &edge).as_bytes(), &Limits::DEFAULT).is_ok(), "genau −1 000 000 ist erlaubt");
}

#[test]
fn validate_checks_links_and_positions() {
    let invalid = |change: &dyn Fn(&mut baruch_exchange::ExchangeDocument)| {
        let mut doc = sample_document();
        change(&mut doc);
        matches!(validate(&doc), Err(ExchangeError::Invalid(_)))
    };
    assert!(validate(&sample_document()).is_ok(), "das Beispiel mit Karte ist gültig");
    assert!(invalid(&|d| d.work.map.links[0].to = sample_uuid(999)), "Verbindung zu einem Einfall außerhalb");
    assert!(invalid(&|d| d.work.map.links[0].to = d.work.map.links[0].from.clone()), "Selbstbezug");
    assert!(
        invalid(&|d| {
            let mut twin = d.work.map.links[1].clone();
            twin.uuid = sample_uuid(95);
            std::mem::swap(&mut twin.from, &mut twin.to);
            d.work.map.links.push(twin);
        }),
        "„widerspricht“ ist ungerichtet: umgekehrt ist es dieselbe Verbindung"
    );
    assert!(invalid(&|d| d.work.map.links[0].note = "x".repeat(501)), "Notiz über 500 Zeichen");
    assert!(invalid(&|d| d.work.map.links[0].note = "Tab\tdarin".into()), "Steuerzeichen außer Zeilenumbruch");
    assert!(invalid(&|d| d.work.map.links[0].uuid = sample_uuid(40)), "Kennung eines Einfalls doppelt vergeben");
    assert!(invalid(&|d| d.work.map.positions[1].idea = sample_uuid(40)), "zwei Lagen für denselben Einfall");
    assert!(invalid(&|d| d.work.map.positions[0].y = 1.0e6 + 1.0), "Koordinate über der Grenze");
    assert!(invalid(&|d| d.work.deleted[1].uuid = sample_uuid(150)), "Grabstein für eine vorhandene Verbindung");
    // Gerichtete Arten dürfen beide Richtungen haben: Das sind zwei verschiedene Verbindungen.
    let mut both = sample_document();
    let mut back = both.work.map.links[0].clone();
    back.uuid = sample_uuid(96);
    std::mem::swap(&mut back.from, &mut back.to);
    both.work.map.links.push(back);
    assert!(validate(&both).is_ok(), "„führt weiter“ in beide Richtungen ist erlaubt");
}

#[test]
fn the_sample_with_its_map_survives_writing_and_reading_exactly() {
    let doc = sample_document();
    // Nur-XML und Container ergeben beim Lesen genau das Dokument (Koordinate 0.1+0.2 bit-genau, −1e6 auf der Grenze).
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    assert!(xml.contains("y=\"0.30000000000000004\""), "kürzeste genaue Darstellung von 0.1+0.2");
    assert!(xml.contains("x=\"-1000000\""), "−1e6 ohne Nachkommastellen");
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(back.work, doc.work, "Werk samt Karte ohne Verlust (XML)");
    assert!(back.unknown.is_empty(), "nichts Unbekanntes: {:?}", back.unknown);
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "Werk samt Karte ohne Verlust (Container)");
}

// ---- Unterversion 3 (Runde 27): Umschlag-Anordnung, Anschriften, Satzbild ----------------------------

/// Grabstein `umschlag-block` des Beispiels (sample_uuid(163)) als Zeile im geschriebenen XML.
const COVER_TOMBSTONE: &str = "\n    <eintrag uuid=\"26000000-0000-4000-8000-0000000000a3\" art=\"umschlag-block\" zeit=\"2026-01-07T19:00:00.000Z\"/>";

/// Die beiden Anschriften des Beispiels als Zeilen im geschriebenen XML.
const ADDRESS_LINES: [&str; 2] = [
    "    <verlag-anschrift>Probeverlag\nAm Brunnen 1\n12345 Musterstadt</verlag-anschrift>\n",
    "    <druckerei-anschrift>Probedruck\nDruckgasse 2\n12345 Musterstadt</druckerei-anschrift>\n",
];

#[test]
fn version_three_carries_cover_blocks_addresses_and_sheet_exactly() {
    let doc = sample_document();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    // Wörter des Formats (Austauschformat.md §2): Farbe hell/dunkel, Laufrichtung, Text des Mottos maskiert.
    for needle in [
        "art=\"vorderseite-titel\"",
        "farbe=\"hell\"",
        "farbe=\"dunkel\"",
        "richtung=\"aufwaerts\"",
        "x-mm=\"-2.5\"",
        "<text>Erfundenes Motto &amp; &lt;Spruch&gt;\nzweite Zeile</text>",
        "<satzbild schrift=\"&quot;Erfundene Antiqua&quot;, serif\" groesse=\"11pt\" blattfarbe=\"#fbf8f1\"/>",
        ADDRESS_LINES[0],
    ] {
        assert!(xml.contains(needle), "{needle} im XML der Unterversion 3:\n{xml}");
    }
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 3 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work.book, doc.work.book, "Buchangaben samt Anschriften und Blöcken ohne Verlust");
    assert_eq!(back.work.sheet, doc.work.sheet, "Satzbild ohne Verlust");
    assert!(
        back.work.deleted.iter().any(|t| t.kind == ExchangeTombstoneKind::CoverBlock),
        "Grabstein umschlag-block gelesen"
    );
}

#[test]
fn files_of_version_two_leave_the_new_parts_unknown_and_refuse_the_new_tombstone() {
    // Ohne die Teile der Unterversion 4 (eigener Test unten), damit nur die Teile der Unterversion 3 übrig bleiben.
    let xml = write_xml(&without_u4(sample_document()), ImageMode::Inline).unwrap();
    assert!(xml.contains(COVER_TOMBSTONE), "Vorbedingung: Grabstein umschlag-block im XML");
    // Eine Datei, die behauptet, Unterversion 2 zu sein, aber die neuen Teile trägt: Der Leser liest sie wie ein
    // Leser der Unterversion 2 – die Teile sind unbekannt, übernommen wird nichts davon.
    let v2 = xml.replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"2\"", 1).replace(COVER_TOMBSTONE, "");
    let doc = read_xml(v2.as_bytes(), &Limits::DEFAULT).unwrap();
    for path in
        ["werk/angaben/satzbild", "werk/buch/verlag-anschrift", "werk/buch/druckerei-anschrift", "werk/buch/umschlag"]
    {
        assert!(doc.unknown.iter().any(|u| u == path), "{path} in Unterversion 2 unbekannt: {:?}", doc.unknown);
    }
    let book = doc.work.book.as_ref().unwrap();
    assert!(book.cover_blocks.is_empty(), "keine Blöcke übernommen");
    assert_eq!((book.publisher_address.as_str(), book.printer_address.as_str()), ("", ""), "Anschriften leer");
    assert_eq!(doc.work.sheet, None, "kein Satzbild übernommen");
    // Den Grabstein umschlag-block gibt es erst ab Unterversion 3 (wie verbindung/lage erst ab 2).
    let with_stone = xml.replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"2\"", 1);
    let result = read_xml(with_stone.as_bytes(), &Limits::DEFAULT);
    assert!(
        matches!(&result, Err(ExchangeError::Xml(m)) if m.contains("umschlag-block") && m.contains("Unterversion 3")),
        "umschlag-block in Unterversion 2 ist ein Fehler: {result:?}"
    );
    // Eine echte Ausgabe der Unterversion 2 (ohne neue Teile) liest sich weiter ohne Hinweis.
    let mut old = without_u4(sample_document());
    old.work.sheet = None;
    old.work.book.as_mut().unwrap().cover_blocks.clear();
    old.work.deleted.retain(|t| t.kind.since_version() <= 2);
    let plain = write_xml(&old, ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"2\"", 1)
        .replace(ADDRESS_LINES[0], "")
        .replace(ADDRESS_LINES[1], "");
    let doc = read_xml(plain.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "u2-Ausgabe ohne neue Teile: nichts unbekannt: {:?}", doc.unknown);
    assert_eq!(doc.version, 2, "Unterversion der Datei bleibt sichtbar");
}

#[test]
fn version_three_without_addresses_reads_them_as_empty() {
    // Das Schema lässt die Anschriften weg (verträglich); der Leser setzt dann leer.
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap().replace(ADDRESS_LINES[0], "");
    let doc = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.work.book.unwrap().publisher_address, "", "fehlende Anschrift = leer");
}

#[test]
fn validate_checks_cover_blocks_and_addresses() {
    let invalid = |change: &dyn Fn(&mut baruch_exchange::ExchangeBook)| {
        let mut doc = sample_document();
        change(doc.work.book.as_mut().unwrap());
        matches!(validate(&doc), Err(ExchangeError::Invalid(_)))
    };
    assert!(
        invalid(&|b| {
            let mut twin = b.cover_blocks[0].clone();
            twin.uuid = sample_uuid(170);
            b.cover_blocks.push(twin);
        }),
        "je Art höchstens ein Block"
    );
    let down = Some(baruch_exchange::ExchangeSpineDirection::Down);
    assert!(invalid(&|b| b.cover_blocks[0].direction = down), "richtung nur am Rücken");
    assert!(invalid(&|b| b.cover_blocks[1].direction = None), "Rücken braucht richtung");
    assert!(invalid(&|b| b.cover_blocks[0].text = Some("Titel".into())), "<text> nur bei eigenem Text");
    assert!(invalid(&|b| b.cover_blocks[2].text = Some("x".repeat(1001))), "Text über 1000 Zeichen");
    assert!(invalid(&|b| b.cover_blocks[0].x_mm = 1000.5), "Lage über 1000 mm");
    assert!(invalid(&|b| b.cover_blocks[0].x_mm = -50.5), "Lage unter −50 mm");
    assert!(invalid(&|b| b.cover_blocks[0].width_mm = 0.5), "Breite unter 1 mm");
    assert!(invalid(&|b| b.cover_blocks[0].size_pt = 201.0), "Größe über 200 pt");
    assert!(invalid(&|b| b.cover_blocks[0].leading_em = 3.5), "Zeilenabstand über 3 em");
    assert!(invalid(&|b| b.cover_blocks[0].font = Some("A;B".into())), "Schriftname mit Semikolon");
    assert!(invalid(&|b| b.cover_blocks[0].uuid = sample_uuid(1)), "Kennung des Werks doppelt vergeben");
    assert!(invalid(&|b| b.publisher_address = "x".repeat(501)), "Anschrift über 500 Zeichen");
    assert!(!invalid(&|b| b.publisher_address = "x".repeat(500)), "genau 500 Zeichen gehen");
    let mut stone = sample_document();
    let last = stone.work.deleted.len() - 1;
    stone.work.deleted[last].uuid = sample_uuid(160);
    assert!(matches!(validate(&stone), Err(ExchangeError::Invalid(_))), "Grabstein für einen vorhandenen Block");
}

// ---- Unterversion 4 (Runde 28): Befunde, alte Beitragsfassungen --------------------------------------------

/// Das Beispiel ohne die Teile der Unterversion 4 (Befunde, Bearbeitungen, Grabsteine ab 4).
/// Runde 29: das Beispiel ohne die Teile der Unterversion 6 (Satz-Einstellungen, Grabsteinart `satz`).
fn without_u6(doc: baruch_exchange::ExchangeDocument) -> baruch_exchange::ExchangeDocument {
    // Runde 31: ohne Unterversion 6 heißt auch ohne Unterversion 7.
    let mut doc = without_u7(doc);
    doc.work.typeset = None;
    doc.work.deleted.retain(|t| t.kind.since_version() <= 5);
    doc
}

/// Runde 31: das Beispiel ohne die Teile der Unterversion 7 (eigene Register, Glossar, Register/Glossar-Felder der
/// Satz-Einstellungen, Grabsteinarten `glossar-begriff` und `register`).
fn without_u7(mut doc: baruch_exchange::ExchangeDocument) -> baruch_exchange::ExchangeDocument {
    doc.work.registers.clear();
    doc.work.glossary.clear();
    if let Some(typeset) = doc.work.typeset.as_mut() {
        typeset.settings.glossary_page = None;
        typeset.settings.index_pages = None;
        typeset.settings.glossary_first_mention = None;
    }
    doc.work.deleted.retain(|t| t.kind.since_version() <= 6);
    doc
}

/// Runde 30: das Beispiel ohne die Teile der Unterversion 5 (Lösungen, Varianten, Schlüssel, Beitrags-Metadaten).
fn without_u5(doc: baruch_exchange::ExchangeDocument) -> baruch_exchange::ExchangeDocument {
    // Runde 29: ohne Unterversion 5 heißt auch ohne Unterversion 6.
    let mut doc = without_u6(doc);
    for finding in &mut doc.work.findings {
        finding.solutions.clear();
    }
    fn clear(nodes: &mut [baruch_exchange::ExchangeNode]) {
        for node in nodes {
            node.key = None;
            for revision in &mut node.revisions {
                revision.variant = None;
            }
            for post in &mut node.posts {
                post.meta = Default::default();
            }
            clear(&mut node.children);
        }
    }
    clear(&mut doc.work.outline);
    for thread in &mut doc.work.threads {
        for post in &mut thread.posts {
            post.meta = Default::default();
        }
    }
    doc
}

fn without_u4(doc: baruch_exchange::ExchangeDocument) -> baruch_exchange::ExchangeDocument {
    // Runde 30: ohne Unterversion 4 heißt auch ohne Unterversion 5.
    let mut doc = without_u5(doc);
    doc.work.findings.clear();
    doc.work.deleted.retain(|t| t.kind.since_version() <= 3);
    fn clear(nodes: &mut [baruch_exchange::ExchangeNode]) {
        for node in nodes {
            for post in &mut node.posts {
                post.edited = None;
                post.versions.clear();
            }
            clear(&mut node.children);
        }
    }
    clear(&mut doc.work.outline);
    for thread in &mut doc.work.threads {
        for post in &mut thread.posts {
            post.edited = None;
            post.versions.clear();
        }
    }
    doc
}

#[test]
fn version_four_carries_findings_history_and_old_post_versions_exactly() {
    let doc = sample_document();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    // Wörter des Formats (Austauschformat.md §2, Unterversion 4): Art, Status, Entscheidung, Prüfvermerk, alte
    // Fassung mit maskiertem Text, Grabsteinarten.
    for needle in [
        "<befunde>",
        "art=\"kritisch\" status=\"verworfen\" persona=\"@lektor\" stelle=\"0\"",
        "<pruefvermerk>(nicht im Text belegt)</pruefvermerk>",
        "<begruendung>Absicht: der Eimer bleibt glatt.\nZweite Zeile.</begruendung>",
        "entscheidung=\"veraltet\"",
        "entscheidung=\"bestaetigt\"",
        "herkunft-art=\"mcp\" herkunft=\"@mcp:probe\" vorschlag=\"true\"",
        "<sammelt befund=",
        "<alte-fassung uuid=",
        "<text>Ja, und die Lampe &amp; &lt;das Seil&gt;.</text>",
        "art=\"befund-status\"",
        "art=\"beitrag-fassung\"",
    ] {
        assert!(xml.contains(needle), "{needle} im XML der Unterversion 4:\n{xml}");
    }
    assert!(xml.find("<karte>") < xml.find("<befunde>"), "<befunde> steht hinter <karte> am Ende von <werk>");
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 4 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work.findings, doc.work.findings, "Befunde samt Verlauf ohne Verlust");
    assert_eq!(back.work.outline, doc.work.outline, "Beiträge samt alten Fassungen ohne Verlust");
    assert_eq!(back.work.threads, doc.work.threads, "Fadenbeiträge samt alten Fassungen ohne Verlust");
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "auch im Container ohne Verlust");
    assert!(validate(&back).is_ok(), "das gelesene Dokument ist gültig");
}

#[test]
fn files_of_version_three_leave_findings_unknown_and_refuse_their_tombstones() {
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    let mut no_stones = without_u5(sample_document());
    no_stones.work.deleted.retain(|t| t.kind.since_version() <= 3);
    // Eine Datei, die behauptet, Unterversion 3 zu sein, aber Befunde und Bearbeitungen trägt: wie ein Leser der
    // Unterversion 3 – unbekannt, nichts übernommen.
    let v3 = write_xml(&no_stones, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"3\"",
        1,
    );
    let doc = read_xml(v3.as_bytes(), &Limits::DEFAULT).unwrap();
    for path in
        ["werk/befunde", "werk/gliederung/punkt/punkt/beitrag/alte-fassung", "werk/faeden/faden/beitrag/@geaendert"]
    {
        assert!(doc.unknown.iter().any(|u| u == path), "{path} in Unterversion 3 unbekannt: {:?}", doc.unknown);
    }
    assert!(doc.work.findings.is_empty(), "keine Befunde übernommen");
    assert!(doc.work.threads[0].posts.iter().all(|p| p.edited.is_none() && p.versions.is_empty()), "keine Fassungen");
    // Die Grabsteine der Unterversion 4 sind in einer Datei der Unterversion 3 ein Fehler.
    let with_stone = xml.replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"3\"", 1);
    let result = read_xml(with_stone.as_bytes(), &Limits::DEFAULT);
    assert!(
        matches!(&result, Err(ExchangeError::Xml(m)) if m.contains("befund") && m.contains("Unterversion 4")),
        "befund in Unterversion 3 ist ein Fehler: {result:?}"
    );
    // Eine echte Ausgabe der Unterversion 3 liest sich weiter ohne Hinweis.
    let plain = write_xml(&without_u4(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"3\"",
        1,
    );
    let doc = read_xml(plain.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "u3-Ausgabe: nichts unbekannt: {:?}", doc.unknown);
    assert_eq!(doc.version, 3, "Unterversion der Datei bleibt sichtbar");
}

#[test]
fn version_five_carries_solutions_variants_keys_and_post_links_exactly() {
    // Runde 30: Rundreise der Unterversion 5 – Lösungen (mit & und <), Varianten mit setzt-um, Schlüssel der Punkte,
    // Grundlage, Antwort, Wiederholung und Relay an Beitrag und Fadenbeitrag.
    let doc = sample_document();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in [
        "<loesung zitat=\"Der Eimer hält\" ersatz=\"Der Eimer trägt &amp; &lt;hält&gt;\"/>",
        "<loesung schritt=\"Bild am Anfang setzen.\"/>",
        "variante=\"A\"",
        "variante-zeile=\"knapp: nur „Klang“ &amp; &lt;Bild&gt;\"",
        "<setzt-um befund=",
        "schluessel=\"11\"",
        "grundlage=\"geprüft: aktuelle Fassung 1\"",
        "relay-von=\"@lektor\"",
    ] {
        assert!(xml.contains(needle), "{needle} steht in der Ausgabe: {xml}");
    }
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "nichts unbekannt: {:?}", back.unknown);
    assert_eq!(back.work, doc.work, "Unterversion 5 verlustfrei (Lösungen, Varianten, Metadaten, Schlüssel)");
}

#[test]
fn files_of_version_four_leave_the_u5_parts_unknown_and_still_read() {
    // Eine Datei, die behauptet, Unterversion 4 zu sein, aber Teile der Unterversionen 5 und 6 trägt: wie ein Leser
    // der Unterversion 4 – unbekannt gemeldet, nichts davon übernommen. (Ohne den Grabstein `satz`, den es erst ab 6
    // gibt; er wäre ein Fehler, siehe Test zur Unterversion 6.)
    let mut doc = without_u7(sample_document());
    doc.work.deleted.retain(|t| t.kind.since_version() <= 5);
    let v4 = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"4\"",
        1,
    );
    let doc = read_xml(v4.as_bytes(), &Limits::DEFAULT).unwrap();
    for path in [
        "werk/befunde/befund/loesung",
        "werk/gliederung/punkt/@schluessel",
        "werk/faeden/faden/beitrag/@relay-von",
        "werk/angaben/satz",
    ] {
        assert!(doc.unknown.iter().any(|u| u == path), "{path} in Unterversion 4 unbekannt: {:?}", doc.unknown);
    }
    assert_eq!(doc.work.typeset, None, "keine Satz-Einstellungen übernommen");
    assert!(doc.work.findings.iter().all(|f| f.solutions.is_empty()), "keine Lösungen übernommen");
    // Eine echte Ausgabe der Unterversion 4 (ohne die neuen Teile) liest sich weiter ohne Hinweis.
    let plain = write_xml(&without_u5(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"4\"",
        1,
    );
    let doc = read_xml(plain.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "u4-Ausgabe: nichts unbekannt: {:?}", doc.unknown);
    assert_eq!(doc.version, 4, "Unterversion der Datei bleibt sichtbar");
}

#[test]
fn validate_checks_variants_post_links_and_solutions() {
    let invalid = |change: &dyn Fn(&mut baruch_exchange::ExchangeWork)| {
        let mut doc = sample_document();
        change(&mut doc.work);
        matches!(validate(&doc), Err(ExchangeError::Invalid(_)))
    };
    /// Der Punkt „Der Eimer“ (der einzige mit Varianten).
    fn bucket(w: &mut baruch_exchange::ExchangeWork) -> &mut baruch_exchange::ExchangeNode {
        fn find(nodes: &mut [baruch_exchange::ExchangeNode]) -> Option<&mut baruch_exchange::ExchangeNode> {
            for n in nodes {
                if n.revisions.iter().any(|r| r.variant.is_some()) {
                    return Some(n);
                }
                if let Some(found) = find(&mut n.children) {
                    return Some(found);
                }
            }
            None
        }
        find(&mut w.outline).expect("Punkt mit Varianten")
    }
    assert!(!invalid(&|_| {}), "das Beispiel ist gültig");
    assert!(invalid(&|w| bucket(w).revisions[2].variant.as_mut().unwrap().label = "D".into()), "Etikett nur A, B, C");
    assert!(invalid(&|w| bucket(w).revisions[2].variant.as_mut().unwrap().group = "x".into()), "Gruppe ist eine UUID");
    assert!(
        invalid(&|w| bucket(w).revisions[2].variant.as_mut().unwrap().findings = vec![sample_uuid(999)]),
        "setzt-um nur auf Befunde des Dokuments"
    );
    assert!(
        invalid(&|w| bucket(w).revisions[2].variant.as_mut().unwrap().summary = "x".repeat(301)),
        "Zeile höchstens 300 Zeichen (CHECK 0047)"
    );
    assert!(
        invalid(&|w| bucket(w).posts[0].meta.basis_revision = Some(sample_uuid(121))),
        "Grundlage nur eine Fassung desselben Punkts"
    );
    assert!(invalid(&|w| bucket(w).posts[1].meta.reply_to = Some(sample_uuid(31))), "Antwort nicht auf sich selbst");
    assert!(invalid(&|w| bucket(w).posts[1].meta.relay_from = Some(String::new())), "relay-von nicht leer");
    assert!(
        invalid(&|w| w.threads[0].posts[1].meta.basis_revision = Some(sample_uuid(111))),
        "ein Fadenbeitrag hat keine grundlage-fassung"
    );
    assert!(
        invalid(&|w| w.findings[1].solutions[0].step = String::new()),
        "eine Lösung braucht Zitat mit Ersatz oder einen Schritt"
    );
}

#[test]
fn validate_checks_findings_like_the_database_does() {
    let invalid = |change: &dyn Fn(&mut baruch_exchange::ExchangeWork)| {
        let mut doc = sample_document();
        change(&mut doc.work);
        matches!(validate(&doc), Err(ExchangeError::Invalid(_)))
    };
    use baruch_exchange::{ExchangeFindingStatus as F, ExchangeOriginKind};
    assert!(!invalid(&|_| {}), "das Beispiel ist gültig");
    assert!(invalid(&|w| w.findings[0].reason = "  ".into()), "verworfen ohne Begründung (CHECK 0046)");
    assert!(invalid(&|w| w.findings[1].adopted_revision = None), "übernommen ohne Fassung");
    assert!(invalid(&|w| w.findings[2].deferred_node = Some(sample_uuid(12))), "verschoben mit zwei Zielen");
    assert!(invalid(&|w| w.findings[2].deferred_idea = None), "verschoben ohne Ziel");
    assert!(invalid(&|w| w.findings[3].post = Some(sample_uuid(30))), "Beitrag und Fadenbeitrag zugleich");
    assert!(invalid(&|w| w.findings[0].post = Some(sample_uuid(999))), "Beitrag nicht im Dokument");
    assert!(invalid(&|w| w.findings[0].node = Some(sample_uuid(998))), "Punkt nicht im Dokument");
    assert!(invalid(&|w| w.findings[3].similar_to = Some(sample_uuid(999))), "gleicht einem fremden Befund");
    assert!(invalid(&|w| w.findings[0].body = " ".into()), "leerer Text");
    assert!(invalid(&|w| w.findings[0].body = "x".repeat(4001)), "Text über 4000 Zeichen");
    assert!(invalid(&|w| w.findings[0].persona = String::new()), "Persona leer");
    assert!(invalid(&|w| w.findings[0].uuid = sample_uuid(30)), "Kennung eines Beitrags doppelt");
    assert!(
        invalid(&|w| {
            w.findings[2].history[0].actor_kind = ExchangeOriginKind::Persona;
        }),
        "ohne Vorschlag setzt nur der Mensch"
    );
    assert!(invalid(&|w| w.findings[1].history[0].decided = None), "Entscheidung ohne Zeitpunkt");
    assert!(invalid(&|w| w.findings[0].history[1].status = F::Adopted), "übernommen ohne Fassung im Verlauf");
    assert!(
        invalid(&|w| {
            let mut twin = w.findings[0].clone();
            twin.history.clear();
            w.findings.push(twin);
        }),
        "derselbe Befund zweimal"
    );
    let mut stone = sample_document();
    let last = stone.work.deleted.len() - 1;
    stone.work.deleted[last].uuid = sample_uuid(310);
    assert!(matches!(validate(&stone), Err(ExchangeError::Invalid(_))), "Grabstein für einen vorhandenen Befund");
}

// ---- Unterversion 6 (Runde 29): Satz-Einstellungen -------------------------------------------------------

#[test]
fn version_six_carries_every_typeset_field_exactly() {
    // Vertrag R29 §6.6: <satz> in <angaben> hinter <satzbild>, jedes Feld aus §2 darstellbar, Rundreise ohne Verlust.
    let doc = sample_document();
    assert!(doc.work.typeset.is_some(), "Vorbedingung: das Beispiel hat Satz-Einstellungen");
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in [
        "kapitelanfang=\"rechte-seite\"",
        "teil-seite=\"rechts\" teil-rueckseite-leer=\"true\"",
        "seitenformat=\"a5\"",
        "rand-innen-mm=\"20\" rand-aussen-mm=\"15\" rand-oben-mm=\"18\" rand-unten-mm=\"22\" bundsteg-mm=\"5\"",
        "grundschrift=\"Libertinus Serif\"",
        "schriftgroesse-pt=\"10.5\"",
        "zeilenabstand-em=\"0.65\"",
        "absatz=\"einzug\" absatz-em=\"1\"",
        "seitenzahlen=\"unten-aussen\" seitenzahlen-beginn=\"erstes-kapitel\"",
        "kolumnentitel=\"kapitel\"",
        "fussnoten=\"arabisch\" fussnoten-zaehlung=\"je-kapitel\"",
        "schusterjungen-hurenkinder=\"true\"",
        "verweis-seitenzahl=\"true\"",
        "geaendert-von=\"Hanna Probe\"",
        "art=\"satz\"",
    ] {
        assert!(xml.contains(needle), "{needle} im XML der Unterversion 6:\n{xml}");
    }
    assert!(xml.find("<satzbild ") < xml.find("<satz "), "<satz> steht hinter <satzbild>");
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 6 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work.typeset, doc.work.typeset, "Satz-Einstellungen ohne Verlust");
    assert!(back.work.deleted.iter().any(|t| t.kind == ExchangeTombstoneKind::Typeset), "Grabstein satz gelesen");
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "auch im Container ohne Verlust");
}

#[test]
fn files_of_version_five_leave_the_typeset_unknown_and_refuse_its_tombstone() {
    // Eine Datei, die behauptet, Unterversion 5 zu sein, aber <satz> trägt: unbekannt, nichts übernommen.
    let mut no_stone = without_u7(sample_document());
    no_stone.work.deleted.retain(|t| t.kind.since_version() <= 5);
    let v5 = write_xml(&no_stone, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"5\"",
        1,
    );
    let doc = read_xml(v5.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.unknown, ["werk/angaben/satz"], "nur <satz> ist in Unterversion 5 unbekannt");
    assert_eq!(doc.work.typeset, None, "keine Satz-Einstellungen übernommen");
    // Den Grabstein satz gibt es erst ab Unterversion 6 (Vertrag §6.6: in Dateien mit version 1–5 ein Fehler).
    for version in 1..=5 {
        // Nur Grabsteine, die es in dieser Unterversion schon gibt, und dazu der Grabstein satz.
        let mut doc = without_u7(sample_document());
        doc.work.deleted.retain(|t| t.kind.since_version() <= version || t.kind == ExchangeTombstoneKind::Typeset);
        let old = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
            &format!("version=\"{FORMAT_VERSION}\""),
            &format!("version=\"{version}\""),
            1,
        );
        let result = read_xml(old.as_bytes(), &Limits::DEFAULT);
        assert!(
            matches!(&result, Err(ExchangeError::Xml(m)) if m.contains("„satz“") && m.contains("Unterversion 6")),
            "satz in Unterversion {version} ist ein Fehler: {result:?}"
        );
    }
    // Eine echte Ausgabe der Unterversion 5 (ohne <satz>) liest sich weiter ohne Hinweis.
    let plain = write_xml(&without_u6(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"5\"",
        1,
    );
    let doc = read_xml(plain.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "u5-Ausgabe: nichts unbekannt: {:?}", doc.unknown);
    assert_eq!(doc.version, 5, "Unterversion der Datei bleibt sichtbar");
}

#[test]
fn broken_typeset_attributes_are_xml_errors_with_their_path() {
    // Halbe zusammengesetzte Felder und fremde Wörter verlören still einen Wert; der Leser lehnt die Datei ab.
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    for (what, broken, expected) in [
        ("halbe Ränder", xml.replace(" bundsteg-mm=\"5\"", ""), "werk/angaben/satz/@bundsteg-mm"),
        ("fremdes Wort", xml.replace("kolumnentitel=\"kapitel\"", "kolumnentitel=\"seite\""), "@kolumnentitel"),
        ("ohne uuid", xml.replacen("<satz uuid=", "<satz kennung=", 1), "werk/angaben/satz/@uuid"),
    ] {
        let result = read_xml(broken.as_bytes(), &Limits::DEFAULT);
        assert!(
            matches!(&result, Err(ExchangeError::Xml(m)) if m.contains(expected)),
            "{what}: Fehler mit {expected}: {result:?}"
        );
    }
    // Eine doppelte Kennung (Satz-Einstellungen = Werk) ist ein Fehler der Prüfung.
    let mut doc = sample_document();
    doc.work.typeset.as_mut().unwrap().uuid = doc.work.uuid.clone();
    assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(m)) if m.contains("Satz-Einstellungen")), "doppelt");
}

// ---- Unterversion 7 (Runde 31): eigene Register, Glossar, Register/Glossar-Felder ----------------------------

#[test]
fn version_seven_carries_registers_glossary_and_the_new_typeset_fields_exactly() {
    // Vertrag R31 (Aufgabe glossar-daten): Glossar mit Varianten und Status, eigene Register, neue <satz>-Attribute,
    // Grabsteinarten glossar-begriff und register – Rundreise ohne Verlust, auch mit & und < im Text.
    let doc = sample_document();
    assert_eq!((doc.work.registers.len(), doc.work.glossary.len()), (2, 3), "Vorbedingung: Beispiel mit u7-Teilen");
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in [
        // Runde 22: Der Schreiber schreibt immer die eigene Unterversion (8); die u7-Teile stehen weiter so da.
        // Geschrieben wird die eigene Unterversion (seit Runde 34 die 9).
        "version=\"9\"",
        "<register uuid=\"26000000-0000-4000-8000-000000000456\" kennung=\"orte\" titel=\"Orte im Hof\"",
        "titel=\"Werkzeuge &amp; Geräte\"",
        "<begriff uuid=\"26000000-0000-4000-8000-00000000044c\" status=\"angenommen\" herkunft=\"Hanna Probe\"",
        "<wort>Brunnenstube</wort>",
        "<variante>Brunnenstuben</variante>",
        "<bibelstelle>Gen 26,19</bibelstelle>",
        "<ausfuehrlich>Im Hof liegt sie unter der Linde.\nDer Deckel ist aus Eiche.</ausfuehrlich>",
        "status=\"vorgeschlagen\" herkunft=\"@lektor\"",
        "<kurz>Sammelbecken für Regenwasser &lt;kein Markup&gt; &amp; Co.</kurz>",
        "glossarseite=\"vor-registern\"",
        "registerseiten=\"stichwort bibel\" registerseiten-spalten=\"2\"",
        "glossar-erste-erwaehnung=\"true\"",
        "art=\"glossar-begriff\"",
        "art=\"register\"",
    ] {
        assert!(xml.contains(needle), "{needle} im XML der Unterversion 7:\n{xml}");
    }
    assert!(xml.find("<eigene-register>") < xml.find("<glossar>"), "Register vor dem Glossar");
    assert!(xml.find("<befunde>") < xml.find("<eigene-register>"), "beide am Ende von <werk> hinter <befunde>");
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 7 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work.registers, doc.work.registers, "eigene Register ohne Verlust, in ihrer Reihenfolge");
    assert_eq!(back.work.glossary, doc.work.glossary, "Glossar ohne Verlust");
    assert_eq!(back.work.typeset, doc.work.typeset, "Satz-Einstellungen samt R31-Feldern ohne Verlust");
    assert_eq!(back.work, doc.work, "ganzes Werk ohne Verlust");
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "auch im Container ohne Verlust");
}

#[test]
fn files_of_version_six_leave_the_u7_parts_unknown_and_refuse_their_tombstones() {
    // Eine Datei, die behauptet, Unterversion 6 zu sein, aber Teile der Unterversion 7 trägt: unbekannt gemeldet,
    // nichts davon übernommen (auch nicht die neuen Attribute an <satz>).
    let mut doc = sample_document();
    doc.work.deleted.retain(|t| t.kind.since_version() <= 6);
    let v6 = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"6\"",
        1,
    );
    let read6 = read_xml(v6.as_bytes(), &Limits::DEFAULT).unwrap();
    for path in [
        "werk/eigene-register",
        "werk/glossar",
        "werk/angaben/satz/@glossarseite",
        "werk/angaben/satz/@registerseiten",
        "werk/angaben/satz/@registerseiten-spalten",
        "werk/angaben/satz/@glossar-erste-erwaehnung",
    ] {
        assert!(read6.unknown.iter().any(|u| u == path), "{path} in Unterversion 6 unbekannt: {:?}", read6.unknown);
    }
    assert!(read6.work.glossary.is_empty() && read6.work.registers.is_empty(), "nichts übernommen");
    let settings = &read6.work.typeset.as_ref().expect("<satz> gilt in Unterversion 6").settings;
    assert_eq!(
        (settings.glossary_page, settings.index_pages.clone(), settings.glossary_first_mention),
        (None, None, None),
        "R31-Felder nicht übernommen"
    );
    assert_eq!(settings.chapter_start, doc.work.typeset.as_ref().unwrap().settings.chapter_start, "Rest gelesen");
    // Die Grabsteine der Unterversion 7 sind in Dateien der Unterversionen 1 bis 6 ein Fehler.
    for (stone, word) in
        [(ExchangeTombstoneKind::GlossaryTerm, "glossar-begriff"), (ExchangeTombstoneKind::Register, "register")]
    {
        let mut doc = without_u7(sample_document());
        let grave = sample_document().work.deleted.into_iter().find(|t| t.kind == stone).unwrap();
        doc.work.deleted.push(grave);
        let old = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
            &format!("version=\"{FORMAT_VERSION}\""),
            "version=\"6\"",
            1,
        );
        let result = read_xml(old.as_bytes(), &Limits::DEFAULT);
        assert!(
            matches!(&result, Err(ExchangeError::Xml(m)) if m.contains(&format!("„{word}“")) && m.contains("Unterversion 7")),
            "{word} in Unterversion 6 ist ein Fehler: {result:?}"
        );
    }
    // Eine echte Ausgabe der Unterversion 6 (ohne die neuen Teile) liest sich weiter ohne Hinweis.
    let plain = write_xml(&without_u7(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"6\"",
        1,
    );
    let doc = read_xml(plain.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(doc.unknown.is_empty(), "u6-Ausgabe: nichts unbekannt: {:?}", doc.unknown);
    assert_eq!(doc.version, 6, "Unterversion der Datei bleibt sichtbar");
}

#[test]
fn validate_checks_registers_and_glossary() {
    // Was sich nicht einlesen ließe, lehnt schon die Prüfung ab (Austauschformat.md, Runde 31).
    type Case = (&'static str, fn(&mut baruch_exchange::ExchangeWork));
    let cases: [Case; 6] = [
        ("eingebaute Kennung", |w| w.registers[0].slug = "bibel".into()),
        ("ungültige Kennung", |w| w.registers[0].slug = "Orte".into()),
        ("Kennung doppelt", |w| w.registers[1].slug = w.registers[0].slug.clone()),
        ("uuid doppelt", |w| w.glossary[1].uuid = w.registers[0].uuid.clone()),
        ("angenommener Begriff doppelt", |w| w.glossary[1].term = " BRUNNENSTUBE ".into()),
        ("leerer Begriff", |w| w.glossary[0].term = "  ".into()),
    ];
    for (what, break_it) in cases {
        let mut doc = sample_document();
        break_it(&mut doc.work);
        assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(_))), "{what}: Invalid erwartet");
    }
    // Ein Vorschlag mit dem Schlüssel eines angenommenen Begriffs ist erlaubt (so steht es auch in der Datenbank).
    let mut doc = sample_document();
    doc.work.glossary[2].term = "brunnenstube".into();
    assert!(validate(&doc).is_ok(), "Vorschlag neben angenommenem Begriff gleichen Schlüssels");
    let mut doc = sample_document();
    doc.work.glossary[0].uuid = sample_uuid(1103);
    assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(_))), "Grabstein für einen vorhandenen Begriff");
}

// ---- Unterversion 8 (Runde 22): Fortschreibungen, Entwurf, Stand an Befunden -------------------------------

#[test]
fn version_eight_carries_amends_stands_and_the_draft_exactly() {
    // R22 §2.11 (a)–(c): Fortschreibungen samt Stand und beruht-stand, Entwurf in <satz>, Stand an Befund und
    // Verlaufseintrag, Grabsteinart fortschreibung – Rundreise ohne Verlust über XML und Container.
    let doc = baruch_exchange::sample::sample_document_u8();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in [
        // Geschrieben wird immer die eigene Unterversion (seit Runde 34 die 9); die Teile der 8 stehen weiter darin.
        "version=\"9\"",
        "<fortschreibung uuid=\"26000000-0000-4000-8000-000000000898\" seq=\"1\" art=\"amend\" autor=\"Hanna Probe\"",
        "seq=\"2\" art=\"undo\" undo-von=\"1\"",
        "<nachher>== Der Eimer\n\nZweiter Entwurf &amp; &lt;mehr&gt;.\n</nachher>",
        "stand=\"3\"",
        "beruht-stand=\"2\"",
        "<entwurf text=\"Probe &amp; &lt;Satz&gt;\" zusatz=\"Stand &quot;heute&quot; · erfunden\" datum=\"true\" \
         deckkraft=\"20\" lage=\"fusszeile\" umschlag=\"true\"/>",
        "art=\"fortschreibung\"",
    ] {
        assert!(xml.contains(needle), "{needle} im XML der Unterversion 8:\n{xml}");
    }
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 8 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work, doc.work, "ganzes Werk ohne Verlust (Fortschreibungen, Stände, Entwurf)");
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "auch über den Container ohne Verlust");
    assert_eq!(write_xml(&back, ImageMode::Inline).unwrap(), xml, "erneut geschrieben byte-gleich");
}

#[test]
fn files_of_version_seven_leave_the_u8_parts_unknown_and_refuse_the_amend_tombstone() {
    // Eine Datei, die Unterversion 7 behauptet, aber u8-Teile trägt: gemeldet, nichts davon übernommen.
    let mut doc = baruch_exchange::sample::sample_document_u8();
    doc.work.deleted.retain(|t| t.kind.since_version() <= 7);
    let v7 = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"7\"",
        1,
    );
    let read7 = read_xml(v7.as_bytes(), &Limits::DEFAULT).unwrap();
    for path in [
        "werk/angaben/satz/entwurf",
        "werk/gliederung/punkt/punkt/fassung/fortschreibung",
        "werk/gliederung/punkt/punkt/fassung/@stand",
        "werk/gliederung/punkt/punkt/fassung/@beruht-stand",
        "werk/befunde/befund/@stand",
        "werk/befunde/befund/status-eintrag/@stand",
    ] {
        assert!(read7.unknown.iter().any(|u| u == path), "{path} in Unterversion 7 unbekannt: {:?}", read7.unknown);
    }
    assert!(read7.work.typeset.as_ref().is_some_and(|t| t.settings.draft.is_none()), "Entwurf nicht übernommen");
    assert!(read7.work.findings.iter().all(|f| f.adopted_amend.is_none()), "Stand am Befund nicht übernommen");
    // Der Grabstein der Unterversion 8 ist in einer Datei der Unterversion 7 ein Fehler.
    let mut doc = sample_document();
    doc.work.deleted.push(ExchangeTombstone {
        uuid: sample_uuid(2210),
        kind: ExchangeTombstoneKind::Amend,
        deleted: "2026-01-08T18:00:00.000Z".into(),
    });
    let old = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"7\"",
        1,
    );
    let err = read_xml(old.as_bytes(), &Limits::DEFAULT).unwrap_err();
    assert!(err.to_string().contains("fortschreibung"), "Fehler nennt die Art: {err}");
}

#[test]
fn validate_refuses_amends_with_gaps_wrong_undo_or_a_wrong_stand() {
    // Was das XSD nicht prüft, prüft der Leser: Schritte lückenlos ab 1, undo-von genau bei undo und kleiner als seq,
    // stand = Zahl der Schritte, Kennungen eindeutig, Autor 1–200 Zeichen.
    let good = baruch_exchange::sample::sample_document_u8();
    validate(&good).expect("das Beispiel ist gültig");
    let edit = |f: &dyn Fn(&mut ExchangeRevision)| {
        let mut doc = good.clone();
        let bucket = &mut doc.work.outline[0].children[0];
        let revision = bucket.revisions.iter_mut().find(|r| !r.amends.is_empty()).expect("Fassung mit Verlauf");
        f(revision);
        doc
    };
    for (what, doc) in [
        ("Lücke", edit(&|r| r.amends[1].seq = 5)),
        ("undo ohne undo-von", edit(&|r| r.amends[1].undo_of = None)),
        ("undo-von nicht früher", edit(&|r| r.amends[1].undo_of = Some(2))),
        ("amend mit undo-von", edit(&|r| r.amends[0].undo_of = Some(1))),
        ("Stand", edit(&|r| r.amend_seq = 2)),
        ("doppelte Kennung", edit(&|r| r.amends[2].uuid = r.amends[0].uuid.clone())),
        // PM-Abschluss R22: Autor wie der CHECK der Migration 0050 (1–200 Zeichen), sonst bräche erst das Schreiben ab.
        ("leerer Autor", edit(&|r| r.amends[0].author = "  ".into())),
        ("Autor zu lang", edit(&|r| r.amends[0].author = "A".repeat(201))),
    ] {
        assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(_))), "{what}: abgelehnt");
    }
}

// ---- Unterversion 9 (Runde 34): Baum der Mindmap am Einfall ------------------------------------------------

#[test]
fn version_nine_carries_title_parent_place_and_main_topic_of_ideas_exactly() {
    // Vertrag R34 §1: <einfall> trägt wahlfrei titel, eltern (UUID des Eltern-Einfalls) und platz; dazu hauptthema
    // (Nachtrag 2026-11-25: ohne Elternknoten, Bezug und Hauptthema liegt ein Einfall in der Wolke).
    let doc = baruch_exchange::sample::sample_document_u9();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    let rope = sample_uuid(40);
    for needle in [
        "version=\"9\"".to_owned(),
        "titel=\"Das Seil &amp; sein &lt;Knoten&gt;\" platz=\"1\"".to_owned(),
        format!("titel=\"Lampe am Seil\" eltern=\"{rope}\" platz=\"2\""),
        "platz=\"3\" hauptthema=\"true\"".to_owned(),
    ] {
        assert!(xml.contains(&needle), "{needle} im XML der Unterversion 9:\n{xml}");
    }
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert!(back.unknown.is_empty(), "in Unterversion 9 ist alles bekannt: {:?}", back.unknown);
    assert_eq!(back.work, doc.work, "ganzes Werk ohne Verlust (Titel, Elternknoten, Platz, Hauptthema)");
    let lamp = &back.work.ideas[1];
    assert_eq!(
        (lamp.title.as_str(), lamp.parent.as_deref(), lamp.position, lamp.main_topic),
        ("Lampe am Seil", Some(rope.as_str()), 2, false),
        "der Unterknoten kennt Titel, Elternknoten und Platz"
    );
    let zip = read(&write_baruch(&doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(zip.work, doc.work, "auch über den Container ohne Verlust");
    assert_eq!(write_xml(&back, ImageMode::Inline).unwrap(), xml, "erneut geschrieben byte-gleich");
}

#[test]
fn an_idea_without_tree_fields_writes_no_new_attributes_and_reads_with_defaults() {
    // Ein Werk ohne Baum sieht in Unterversion 9 aus wie zuvor: Die neuen Attribute stehen nur da, wenn sie von der
    // Vorgabe abweichen – so bleibt eine Ausgabe ohne Mindmap auch nach dem eingefrorenen u8-XSD gültig.
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    let ideas: Vec<&str> = xml.lines().filter(|line| line.trim_start().starts_with("<einfall ")).collect();
    assert_eq!(ideas.len(), 3, "Vorbedingung: die drei Einfälle des Beispiels stehen je in einer Zeile");
    for line in ideas {
        for attribute in [" titel=", " eltern=", " platz=", " hauptthema="] {
            assert!(!line.contains(attribute), "{attribute} fehlt ohne Baum: {line}");
        }
    }
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    for idea in &back.work.ideas {
        assert_eq!(
            (idea.title.as_str(), idea.parent.as_deref(), idea.position, idea.main_topic),
            ("", None, 0, false),
            "fehlende Attribute lesen sich als Vorgabe (kein Titel, kein Elternknoten, Platz 0, kein Hauptthema)"
        );
    }
}

#[test]
fn files_of_version_eight_leave_the_tree_attributes_unknown_and_take_nothing_of_them() {
    // Eine Datei, die Unterversion 8 behauptet, aber die Attribute der 9 trägt: gemeldet, nichts davon übernommen;
    // alles Übrige der Datei wird gelesen (Dateien der Unterversionen 1 bis 8 bleiben lesbar).
    let doc = baruch_exchange::sample::sample_document_u9();
    let v8 = write_xml(&doc, ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"8\"",
        1,
    );
    let read8 = read_xml(v8.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(read8.version, 8, "die Unterversion der Datei bleibt sichtbar");
    for path in [
        "werk/einfaelle/einfall/@titel",
        "werk/einfaelle/einfall/@eltern",
        "werk/einfaelle/einfall/@platz",
        "werk/einfaelle/einfall/@hauptthema",
    ] {
        assert!(read8.unknown.iter().any(|u| u == path), "{path} in Unterversion 8 unbekannt: {:?}", read8.unknown);
    }
    assert!(
        read8.work.ideas.iter().all(|i| i.title.is_empty() && i.parent.is_none() && i.position == 0 && !i.main_topic),
        "nichts vom Baum übernommen"
    );
    assert_eq!(read8.work.ideas.len(), doc.work.ideas.len(), "die Einfälle selbst sind alle gelesen");
    assert_eq!(read8.work.ideas[0].words, doc.work.ideas[0].words, "mit ihren alten Feldern");
}

#[test]
fn validate_refuses_a_long_title_a_broken_parent_and_an_idea_as_its_own_parent() {
    // Was das XSD nicht oder nur teils prüft, prüft der Leser: Titel höchstens 200 Zeichen (wie idea.title), eltern
    // eine UUID und nicht der Einfall selbst. Ein Elternknoten, der nicht im Dokument steht, ist erlaubt – das
    // Einlesen löst den Einfall dann mit Hinweis (Vertrag R34 §1).
    let mut doc = baruch_exchange::sample::sample_document_u9();
    assert!(validate(&doc).is_ok(), "das Beispiel der Unterversion 9 ist gültig");
    doc.work.ideas[1].title = "ä".repeat(200);
    assert!(validate(&doc).is_ok(), "200 Zeichen (nicht Bytes) sind erlaubt");
    doc.work.ideas[1].title = "ä".repeat(201);
    assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(m)) if m.contains("Titel")), "201 Zeichen abgelehnt");
    let mut doc = baruch_exchange::sample::sample_document_u9();
    doc.work.ideas[1].parent = Some("kein-uuid".into());
    assert!(matches!(validate(&doc), Err(ExchangeError::Invalid(_))), "eltern muss eine UUID sein");
    let mut doc = baruch_exchange::sample::sample_document_u9();
    doc.work.ideas[1].parent = Some(doc.work.ideas[1].uuid.clone());
    assert!(
        matches!(validate(&doc), Err(ExchangeError::Invalid(m)) if m.contains("eigener Elternknoten")),
        "ein Einfall ist nicht sein eigener Elternknoten"
    );
    let mut doc = baruch_exchange::sample::sample_document_u9();
    doc.work.ideas[1].parent = Some(sample_uuid(4711));
    assert!(validate(&doc).is_ok(), "ein unbekannter Elternknoten ist kein Formatfehler (das Einlesen löst ihn)");
}

#[test]
fn a_tree_attribute_with_a_wrong_value_is_a_reading_error() {
    let xml = write_xml(&baruch_exchange::sample::sample_document_u9(), ImageMode::Inline).unwrap();
    let broken = xml.replacen("platz=\"2\"", "platz=\"zwei\"", 1);
    assert!(
        matches!(read_xml(broken.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(m)) if m.contains("platz")),
        "platz muss eine ganze Zahl sein; der Fehler nennt das Attribut"
    );
    let broken = xml.replacen("hauptthema=\"true\"", "hauptthema=\"vielleicht\"", 1);
    assert!(
        matches!(read_xml(broken.as_bytes(), &Limits::DEFAULT), Err(ExchangeError::Xml(m)) if m.contains("hauptthema")),
        "hauptthema muss ein Wahrheitswert sein"
    );
}
