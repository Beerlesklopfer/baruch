// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 26 "export": the output passes the XSD; since Round 34 (sub-version 9, mind map) the XSD
//! `docs/schema/baruch-werk-1-u9.xsd`; `baruch-werk-1-u8.xsd` (sub-version 8), `baruch-werk-1-u7.xsd`
//! (sub-version 7), `baruch-werk-1-u6.xsd` (sub-version 6), `baruch-werk-1-u5.xsd` (sub-version 5),
//! `baruch-werk-1-u4.xsd` (sub-version 4), `baruch-werk-1-u3.xsd` (sub-version 3), `baruch-werk-1-u2.xsd`
//! (sub-version 2) and `baruch-werk-1.xsd` (sub-version 1) stay frozen (byte-identical, checked through SHA-256).
//!
//! The check uses `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u9.xsd DATEI` (contract F2). If
//! xmllint is missing, the test reports that honestly and skips only this part; the Rust check of the mandatory
//! elements through quick-xml events (not through the reader of import) always runs.

use std::path::{Path, PathBuf};
use std::process::Command;

use baruch_exchange::sample::sample_document;
use baruch_exchange::write::{ImageMode, write_baruch, write_manifest, write_xml};
use baruch_exchange::*;
use quick_xml::events::Event;

/// Schema of sub-version 9 (Round 34), against which every output is checked.
fn schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd")
}

/// Frozen schema of sub-version 9 (Round 34).
fn schema_u9() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u9.xsd")
}

/// Frozen schema of sub-version 8 (Round 22).
fn schema_u8() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u8.xsd")
}

/// Frozen schema of sub-version 7 (Round 31).
fn schema_u7() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u7.xsd")
}

/// Frozen schema of sub-version 6 (Round 29).
fn schema_u6() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u6.xsd")
}

/// Round 31: the sample without the parts of sub-version 7 (own indexes, glossary, index/glossary fields of the
/// typesetting settings, tombstone kinds `glossar-begriff` and `register`).
fn without_u7(mut doc: ExchangeDocument) -> ExchangeDocument {
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

/// Frozen schema of sub-version 5 (Round 30).
fn schema_u5() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u5.xsd")
}

/// Round 29: the sample without the parts of sub-version 6 (typesetting settings, tombstone kind `satz`).
fn without_u6(doc: ExchangeDocument) -> ExchangeDocument {
    let mut doc = without_u7(doc);
    doc.work.typeset = None;
    doc.work.deleted.retain(|t| t.kind.since_version() <= 5);
    doc
}

/// Frozen schema of sub-version 4 (Round 28).
fn schema_u4() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u4.xsd")
}

/// Round 30: the sample without the parts of sub-version 5 (solutions, variants, keys, post metadata).
fn without_u5(doc: ExchangeDocument) -> ExchangeDocument {
    let mut doc = without_u6(doc);
    for finding in &mut doc.work.findings {
        finding.solutions.clear();
    }
    fn clear(nodes: &mut [ExchangeNode]) {
        for node in nodes {
            node.key = None;
            for revision in &mut node.revisions {
                revision.variant = None;
            }
            for post in &mut node.posts {
                post.meta = ExchangePostMeta::default();
            }
            clear(&mut node.children);
        }
    }
    clear(&mut doc.work.outline);
    for thread in &mut doc.work.threads {
        for post in &mut thread.posts {
            post.meta = ExchangePostMeta::default();
        }
    }
    doc
}

/// Frozen schema of sub-version 3 (Round 27).
fn schema_u3() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u3.xsd")
}

/// The sample without the parts of sub-version 4 (findings, edits of posts, tombstones from 4).
fn without_u4(doc: ExchangeDocument) -> ExchangeDocument {
    let mut doc = without_u5(doc);
    doc.work.findings.clear();
    doc.work.deleted.retain(|t| t.kind.since_version() <= 3);
    fn clear(nodes: &mut [ExchangeNode]) {
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

/// Frozen schema of sub-version 2 (Round 25).
fn schema_u2() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u2.xsd")
}

/// Frozen schema of sub-version 1.
fn schema_v1() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1.xsd")
}

fn xmllint_available() -> bool {
    Command::new("xmllint").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Checks a file with xmllint against the XSD; `false` if xmllint is missing (then with a message).
fn xmllint(name: &str, content: &str) -> bool {
    xmllint_with(&schema(), name, content)
}

/// Like [`xmllint`], against a particular schema.
fn xmllint_with(schema: &Path, name: &str, content: &str) -> bool {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: XSD-Prüfung von {name} übersprungen, es greift nur die Rust-Prüfung.");
        return false;
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(name);
    std::fs::write(&file, content).unwrap();
    let output =
        Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema).arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "{name} besteht das XSD {} nicht:\n{}\n--- Datei ---\n{content}",
        schema.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

/// Rust check of the mandatory elements: root in the namespace, children in the order of the vocabulary,
/// per item a title and at least one revision, per revision exactly one `<text>`, per image revision exactly
/// one of `datei` and `<daten>`.
fn check_required(xml: &str, images: ImageMode) {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    // Per open element: name, names of the children, whether `datei` is set.
    let mut stack: Vec<(String, Vec<String>, bool)> = Vec::new();
    let mut root_seen = false;
    let mut closed: Vec<(String, String, Vec<String>, bool)> = Vec::new();
    loop {
        match reader.read_event().expect("wohlgeformt") {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let (e, empty) = match &event {
                    Event::Start(e) => (e.clone(), false),
                    Event::Empty(e) => (e.clone(), true),
                    _ => unreachable!(),
                };
                let name = e.local_name().into_inner().to_owned();
                if !root_seen {
                    root_seen = true;
                    assert_eq!(name, "werk", "Wurzel ist <werk>");
                    let ns = e.try_get_attribute("xmlns").unwrap().expect("xmlns an der Wurzel");
                    assert_eq!(
                        ns.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap(),
                        NAMESPACE,
                        "Namensraum"
                    );
                    let version = e.try_get_attribute("version").unwrap().expect("version an der Wurzel");
                    assert_eq!(
                        version.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap(),
                        "10",
                        "Unterversion 10 (Runde 38)"
                    );
                    for required in ["uuid", "art"] {
                        assert!(e.try_get_attribute(required).unwrap().is_some(), "werk/@{required}");
                    }
                }
                let datei = e.try_get_attribute("datei").unwrap().is_some();
                if let Some(parent) = stack.last_mut() {
                    parent.1.push(name.clone());
                }
                let parent = stack.last().map(|p| p.0.clone()).unwrap_or_default();
                if empty {
                    closed.push((parent, name, Vec::new(), datei));
                } else {
                    stack.push((name, Vec::new(), datei));
                }
            }
            Event::End(_) => {
                let (name, kids, datei) = stack.pop().unwrap();
                let parent = stack.last().map(|p| p.0.clone()).unwrap_or_default();
                closed.push((parent, name, kids, datei));
            }
            Event::DocType(_) => panic!("keine DTD"),
            Event::Eof => break,
            _ => {}
        }
    }
    let werk = closed.iter().find(|(p, n, _, _)| p.is_empty() && n == "werk").expect("<werk>");
    let order = [
        "ausgabe",
        "angaben",
        "buch",
        "vorlage",
        "gliederung",
        "einfaelle",
        "faeden",
        "bilder",
        "geloescht",
        "karte",
        "befunde",
        "eigene-register",
        "glossar",
    ];
    // Round 31: after <befunde> the own indexes and the glossary (new elements at the end of the sequence, §6).
    assert_eq!(
        &werk.2[werk.2.len() - 3..],
        ["befunde", "eigene-register", "glossar"],
        "das Beispiel hat Befunde, eigene Register und Glossar am Ende von <werk>"
    );
    let positions: Vec<usize> =
        werk.2.iter().map(|k| order.iter().position(|o| o == k).expect("bekanntes Kind von werk")).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "Kinder von werk in Reihenfolge, je höchstens einmal");
    for required in ["ausgabe", "angaben", "vorlage", "gliederung"] {
        assert!(werk.2.iter().any(|k| k == required), "Pflichtelement werk/{required}");
    }
    let details = closed.iter().find(|(p, n, _, _)| p == "werk" && n == "angaben").unwrap();
    for required in ["titel", "untertitel", "autor", "zielgruppe", "bibel", "angelegt", "schema"] {
        assert!(details.2.iter().any(|k| k == required), "Pflichtelement angaben/{required}");
    }
    // Sub-version 3: <satzbild> at the end of <angaben>, since sub-version 6 <satz> after it (new elements at the
    // end of the sequence, §6).
    assert_eq!(&details.2[details.2.len() - 2..], ["satzbild", "satz"], "satzbild und satz am Ende von angaben");
    let book = closed.iter().find(|(p, n, _, _)| p == "werk" && n == "buch").expect("<buch>");
    assert_eq!(
        &book.2[book.2.len() - 4..],
        ["vita", "verlag-anschrift", "druckerei-anschrift", "umschlag"],
        "Anschriften und Umschlag am Ende von buch"
    );
    let cover = closed.iter().find(|(p, n, _, _)| p == "buch" && n == "umschlag").expect("<umschlag>");
    assert_eq!(cover.2, ["block", "block", "block"], "drei Blöcke des Beispiels");
    let texts = closed.iter().filter(|(p, n, _, _)| p == "block" && n == "text").count();
    assert_eq!(texts, 1, "<text> nur beim Block mit eigenem Text (Motto)");
    let mut points = 0;
    let mut image_revisions = 0;
    for (parent, name, kids, datei) in &closed {
        match (parent.as_str(), name.as_str()) {
            ("gliederung" | "punkt", "punkt") => {
                points += 1;
                assert_eq!(kids.first().map(String::as_str), Some("titel"), "punkt beginnt mit <titel>");
                assert!(kids.iter().any(|k| k == "fassung"), "punkt hat mindestens eine Fassung");
            }
            ("punkt" | "vorlage", "fassung") => {
                assert_eq!(kids.iter().filter(|k| *k == "text").count(), 1, "Fassung mit genau einem <text>");
            }
            ("bild", "fassung") => {
                image_revisions += 1;
                let daten = kids.iter().any(|k| k == "daten");
                assert!(daten != *datei, "genau eines von datei und <daten>");
                assert_eq!(*datei, images == ImageMode::Files, "datei nur im ZIP, <daten> nur im Nur-XML");
            }
            _ => {}
        }
    }
    assert_eq!(points, 6, "alle sechs Punkte des Beispiels stehen in der Gliederung");
    let karte = closed.iter().find(|(p, n, _, _)| p == "werk" && n == "karte").expect("<karte>");
    // First all links, then all positions (xs:sequence in the XSD of sub-version 2).
    assert_eq!(karte.2, ["verbindung", "verbindung", "lage", "lage"], "Kinder von <karte> in Reihenfolge");
    assert_eq!(image_revisions, 3, "alle drei Bildfassungen des Beispiels");
}

#[test]
fn inline_xml_passes_the_schema() {
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    check_required(&xml, ImageMode::Inline);
    xmllint("probe.baruch.xml", &xml);
}

#[test]
fn files_xml_passes_the_schema() {
    let xml = write_xml(&sample_document(), ImageMode::Files).unwrap();
    check_required(&xml, ImageMode::Files);
    xmllint("werk.xml", &xml);
}

#[test]
fn manifest_passes_the_schema() {
    // The manifest from a real container (with images), not built by hand.
    let bytes = write_baruch(&sample_document()).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut manifest = String::new();
    std::io::Read::read_to_string(&mut archive.by_name(ENTRY_MANIFEST).unwrap(), &mut manifest).unwrap();
    assert!(manifest.contains(&format!("<manifest xmlns=\"{NAMESPACE}\" version=\"10\">")), "Wurzel im Namensraum");
    xmllint("MANIFEST.xml", &manifest);
    // Also write_manifest alone.
    let entries = [ExchangeManifestEntry { path: ENTRY_WORK.into(), sha256: sha256_hex(b"x"), bytes: 1 }];
    xmllint("einzeln.xml", &write_manifest(&entries).unwrap());
}

#[test]
fn schema_rejects_a_broken_file_so_the_check_has_teeth() {
    // Cross-check: without it, it would go unnoticed if xmllint did not check against the schema at all.
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap().replace("status=\"vorschlag\"", "status=\"x\"");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("kaputt.xml");
    std::fs::write(&file, xml).unwrap();
    let output =
        Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
    assert!(!output.status.success(), "ein unbekannter Status muss am XSD scheitern");
}

#[test]
fn the_frozen_v1_and_u2_schemas_are_byte_unchanged() {
    // Contract R27 1.11: u2 and v1 stay frozen. The checksums are the states of development before Round 27
    // (af07921/cd54cbb); every change to the files would show up here. Since Round 28 u3 is frozen too (state
    // of development cb7b169).
    let sha = |path: PathBuf| sha256_hex(&std::fs::read(path).unwrap());
    // Round 22: u7 is frozen too (state of development dfff467).
    assert_eq!(sha(schema_u7()), U7_SHA256, "baruch-werk-1-u7.xsd byte-unverändert");
    // Round 34: u8 is frozen too (state of development 67faa2f, interface R34-S1).
    assert_eq!(sha(schema_u8()), U8_SHA256, "baruch-werk-1-u8.xsd byte-unverändert");
    // Round 31: u6 is frozen too (state of development 5f97a8b).
    assert_eq!(
        sha(schema_u6()),
        "bab4657a6a971c129c5e4685d2bd8143f41aa18ed5c4cd7ef54ced807cf26d38",
        "baruch-werk-1-u6.xsd byte-unverändert"
    );
    // Round 29: u5 is frozen too (state of development d3a2a8f).
    assert_eq!(
        sha(schema_u5()),
        "106d3a87dbc84351c65c1d1b2496f164ef8037d65c9f2270a9b9a8172627fe17",
        "baruch-werk-1-u5.xsd byte-unverändert"
    );
    // Round 30: u4 is frozen too (state of development aa6808c).
    assert_eq!(
        sha(schema_u4()),
        "6c8833a52d473c0aa786663e427f63afa08d1b4821aa58abdc42b6ebf6b10924",
        "baruch-werk-1-u4.xsd byte-unverändert"
    );
    assert_eq!(
        sha(schema_u3()),
        "2ce5522a189dd9bf469af57845a785c33bff080669c064a8f3408376224aa7b7",
        "baruch-werk-1-u3.xsd byte-unverändert"
    );
    assert_eq!(
        sha(schema_u2()),
        "da6e0d2dc8511b5e9e877efe5bbdb0d620f7275dca87fbbdba36532cf2d4c856",
        "baruch-werk-1-u2.xsd byte-unverändert"
    );
    assert_eq!(
        sha(schema_v1()),
        "f5da7db9fcb4094520682d963f636a187201f4c5465c33a7c43ce592cd21c9e7",
        "baruch-werk-1.xsd byte-unverändert"
    );
}

#[test]
fn the_u3_schema_defines_cover_blocks_addresses_and_sheet() {
    let u3 = std::fs::read_to_string(schema_u3()).unwrap();
    for needle in [
        "name=\"satzbild\"",
        "name=\"verlag-anschrift\"",
        "name=\"druckerei-anschrift\"",
        "name=\"umschlag\"",
        "name=\"UmschlagBlock\"",
        "value=\"umschlag-block\"",
        "value=\"rueckseite-verlag-preis\"",
        "value=\"aufwaerts\"",
        "targetNamespace=\"urn:baruch:werk:1\"",
    ] {
        assert!(u3.contains(needle), "u3-XSD enthält {needle}");
    }
    // Every XML word of the model is in the schema (otherwise an output would not pass the XSD).
    for kind in ExchangeCoverBlockKind::ALL {
        assert!(u3.contains(&format!("value=\"{}\"", kind.as_xml())), "Blockart {} im u3-XSD", kind.as_xml());
    }
    for kind in ExchangeTombstoneKind::ALL.iter().filter(|k| k.since_version() <= 3) {
        assert!(u3.contains(&format!("value=\"{}\"", kind.as_xml())), "Grabsteinart {} im u3-XSD", kind.as_xml());
    }
    assert!(!u3.contains("value=\"befund\""), "u3 kennt keine Befunde (eingefroren)");
}

#[test]
fn the_u4_schema_defines_findings_and_old_post_versions() {
    let u4 = std::fs::read_to_string(schema_u4()).unwrap();
    for needle in [
        "name=\"befunde\"",
        "name=\"Befund\"",
        "name=\"StatusEintrag\"",
        "name=\"alte-fassung\"",
        "name=\"AlteFassung\"",
        "name=\"sammelt\"",
        "name=\"pruefvermerk\"",
        "targetNamespace=\"urn:baruch:werk:1\"",
    ] {
        assert!(u4.contains(needle), "u4-XSD enthält {needle}");
    }
    // Every XML word of the model is in the schema (otherwise an output would not pass the XSD).
    for kind in ExchangeTombstoneKind::ALL.iter().filter(|k| k.since_version() <= 4) {
        assert!(u4.contains(&format!("value=\"{}\"", kind.as_xml())), "Grabsteinart {} im u4-XSD", kind.as_xml());
    }
    assert!(!u4.contains("value=\"satz\""), "u4 kennt keine Satz-Einstellungen (eingefroren)");
    for kind in ExchangeFindingKind::ALL {
        assert!(u4.contains(&format!("value=\"{}\"", kind.as_xml())), "Befundart {} im u4-XSD", kind.as_xml());
    }
    for status in ExchangeFindingStatus::ALL {
        assert!(u4.contains(&format!("value=\"{}\"", status.as_xml())), "Status {} im u4-XSD", status.as_xml());
    }
    for decision in ExchangeDecision::ALL {
        assert!(u4.contains(&format!("value=\"{}\"", decision.as_xml())), "Entscheidung {}", decision.as_xml());
    }
    assert!(!u4.contains("name=\"loesung\""), "u4 kennt keine Lösungen (eingefroren)");
}

#[test]
fn the_u5_schema_defines_solutions_variants_keys_and_post_links() {
    let u5 = std::fs::read_to_string(schema_u5()).unwrap();
    for needle in [
        "name=\"loesung\"",
        "name=\"Loesung\"",
        "name=\"setzt-um\"",
        "name=\"variante-gruppe\"",
        "name=\"variante\"",
        "name=\"variante-zeile\"",
        "name=\"schluessel\"",
        "name=\"grundlage-fassung\"",
        "name=\"grundlage\"",
        "name=\"antwort-auf\"",
        "name=\"wiederholt\"",
        "name=\"relay-von\"",
        "targetNamespace=\"urn:baruch:werk:1\"",
    ] {
        assert!(u5.contains(needle), "u5-XSD enthält {needle}");
    }
    for label in VARIANT_LABELS {
        assert!(u5.contains(&format!("value=\"{label}\"")), "Etikett {label} im u5-XSD");
    }
    for kind in ExchangeTombstoneKind::ALL.iter().filter(|k| k.since_version() <= 5) {
        assert!(u5.contains(&format!("value=\"{}\"", kind.as_xml())), "Grabsteinart {} im u5-XSD", kind.as_xml());
    }
    assert!(!u5.contains("name=\"satz\""), "u5 kennt keine Satz-Einstellungen (eingefroren)");
}

#[test]
fn the_u6_schema_defines_the_typeset_settings() {
    // Contract R29 §6.6: <satz> in <angaben> with every attribute the writer knows, and the tombstone kind satz.
    // Since Round 31 u6 is frozen; the parts up to sub-version 6 are checked.
    let u6 = std::fs::read_to_string(schema_u6()).unwrap();
    for needle in ["name=\"satz\"", "name=\"Satz\"", "name=\"geaendert-von\"", "targetNamespace=\"urn:baruch:werk:1\""]
    {
        assert!(u6.contains(needle), "u6-XSD enthält {needle}");
    }
    let r31 = baruch_exchange::typeset::R31_ATTRIBUTES;
    for attribute in baruch_exchange::typeset::TYPESET_ATTRIBUTES.iter().filter(|a| !r31.contains(a)) {
        assert!(u6.contains(&format!("name=\"{attribute}\"")), "Attribut {attribute} im u6-XSD");
    }
    for attribute in r31 {
        assert!(!u6.contains(&format!("name=\"{attribute}\"")), "{attribute} erst ab u7 (u6 eingefroren)");
    }
    for kind in ExchangeTombstoneKind::ALL.iter().filter(|k| k.since_version() <= 6) {
        assert!(u6.contains(&format!("value=\"{}\"", kind.as_xml())), "Grabsteinart {} im u6-XSD", kind.as_xml());
    }
    // Every word of every enumeration that the writer outputs is in the schema.
    let custom = baruch_typst::typeset::TypesetSettings {
        page_format: Some(baruch_typst::typeset::PageFormat::Custom { width_mm: 130.0, height_mm: 200.0 }),
        glossary_page: None,
        index_pages: None,
        glossary_first_mention: None,
        ..baruch_exchange::typeset::full_sample()
    };
    for (name, value) in baruch_exchange::typeset::settings_attrs(&custom).unwrap() {
        if !value.chars().next().is_some_and(|c| c.is_ascii_digit()) && !["grundschrift", "sprache"].contains(&name) {
            let word = format!("value=\"{value}\"");
            assert!(u6.contains(&word) || value == "true" || value == "false", "{name}={value} im u6-XSD");
        }
    }
}

#[test]
fn the_u6_schema_accepts_a_u5_file_and_the_frozen_u5_schema_too() {
    // Contract R29 §6.6: an output without the novelties of sub-version 6 (as Round 30 wrote it) is valid by u6
    // and by the frozen u5.
    let u5 = write_xml(&without_u6(sample_document()), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"5\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    assert!(!u5.contains("<satz "), "Vorbedingung: ohne <satz>");
    if xmllint_with(&schema(), "u5.baruch.xml", &u5) {
        assert!(xmllint_with(&schema_u5(), "u5.baruch.xml", &u5), "dieselbe Datei gilt nach dem u5-XSD");
    }
    // A u6 output with <satz>, by contrast, is not valid by u5 (otherwise the schema would not be a real extension).
    let u6 = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    if xmllint_available() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("u6.baruch.xml");
        std::fs::write(&file, &u6).unwrap();
        let output = Command::new("xmllint")
            .args(["--noout", "--nonet", "--schema"])
            .arg(schema_u5())
            .arg(&file)
            .output()
            .unwrap();
        assert!(!output.status.success(), "<satz> ist im u5-XSD unbekannt");
    }
}

#[test]
fn the_u6_schema_rejects_broken_typeset_settings() {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe der Unterversion 6 übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xml.contains("kapitelanfang=\"rechte-seite\"") && xml.contains("sprache=\"de\""), "Vorbedingung: {xml}");
    for (what, broken) in [
        ("Kapitelanfang", xml.replace("kapitelanfang=\"rechte-seite\"", "kapitelanfang=\"links\"")),
        ("Sprache", xml.replace("sprache=\"de\"", "sprache=\"Deutsch\"")),
        ("Zahl", xml.replace("schriftgroesse-pt=\"10.5\"", "schriftgroesse-pt=\"groß\"")),
        ("fremdes Attribut", xml.replacen("<satz ", "<satz farbe=\"rot\" ", 1)),
        ("ohne uuid", xml.replacen("<satz uuid=\"", "<satz kennung=\"", 1)),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u6-XSD scheitern");
    }
}

#[test]
fn the_u5_schema_rejects_broken_variants_and_solutions() {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe der Unterversion 5 übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xml.contains("variante=\"A\"") && xml.contains("<loesung "), "Vorbedingung: {xml}");
    for (what, broken) in [
        ("Etikett", xml.replace("variante=\"A\"", "variante=\"D\"")),
        ("Gruppe", xml.replacen("variante-gruppe=\"", "variante-gruppe=\"x", 1)),
        ("Lösung", xml.replacen("<loesung ", "<loesung farbe=\"rot\" ", 1)),
        ("Zeile", xml.replace("variante-zeile=\"gründlich\"", &format!("variante-zeile=\"{}\"", "x".repeat(301)))),
        ("Reihenfolge", xml.replacen("<setzt-um ", "<notiz>x</notiz><setzt-um ", 1)),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u5-XSD scheitern");
    }
}

#[test]
fn the_u5_schema_accepts_a_u4_file_and_the_frozen_u4_schema_too() {
    // An output without the novelties of sub-version 5 (as Round 28 wrote it) is valid by u5 and u4.
    let u4 = write_xml(&without_u5(sample_document()), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"4\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    if xmllint_with(&schema(), "u4.baruch.xml", &u4) {
        assert!(xmllint_with(&schema_u4(), "u4.baruch.xml", &u4), "dieselbe Datei gilt nach dem u4-XSD");
    }
}

#[test]
fn the_u4_schema_rejects_broken_findings() {
    // Cross-check for <befunde>: unknown kind, unknown status, foreign child, persona empty, decision foreign.
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe der Befunde übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xml.contains("art=\"kritisch\" status=\"verworfen\" persona=\"@lektor\""), "Vorbedingung: {xml}");
    for (what, broken) in [
        ("Art", xml.replace("art=\"kritisch\"", "art=\"tadel\"")),
        ("Status", xml.replace("status=\"verworfen\" persona", "status=\"vergessen\" persona")),
        ("Kind", xml.replace("<pruefvermerk>", "<farbe/><pruefvermerk>")),
        ("Persona", xml.replace("persona=\"@lektor\"", "persona=\"\"")),
        ("Entscheidung", xml.replace("entscheidung=\"veraltet\"", "entscheidung=\"vielleicht\"")),
        // <bezug> after <alte-fassung>: the old versions are at the end of the post.
        (
            "Reihenfolge",
            xml.replacen(
                "</alte-fassung>",
                "</alte-fassung><bezug punkt=\"26000000-0000-4000-8000-00000000000b\">x</bezug>",
                1,
            ),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u4-XSD scheitern");
    }
}

#[test]
fn the_frozen_v1_schema_is_unchanged_and_the_u2_schema_defines_the_map() {
    let v1 = std::fs::read_to_string(schema_v1()).unwrap();
    // Sub-version 1 stays frozen: laxly checked <karte>, tombstone kinds without verbindung/lage.
    assert!(v1.contains("processContents=\"lax\""), "v1: <karte> lax wie in Runde 26");
    assert!(!v1.contains("\"verbindung\"") && !v1.contains("\"lage\""), "v1: keine neuen Grabsteinarten");
    let u2 = std::fs::read_to_string(schema_u2()).unwrap();
    for needle in [
        "name=\"karte\"",
        "name=\"Verbindung\"",
        "name=\"Lage\"",
        "value=\"verbindung\"",
        "value=\"lage\"",
        "value=\"fuehrt-weiter\"",
        "value=\"widerspricht\"",
        "targetNamespace=\"urn:baruch:werk:1\"",
    ] {
        assert!(u2.contains(needle), "u2-XSD enthält {needle}");
    }
    assert!(!u2.contains("processContents=\"lax\""), "u2: <karte> ist festgelegt, nicht mehr lax");
}

#[test]
fn the_u3_schema_accepts_every_valid_file_of_versions_one_and_two() {
    // The hand-written test file of sub-version 1 (the other files under tests/data/import are
    // deliberately unsafe – DTD, entities – and are valid in no sub-version).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/import");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        if !text.contains("version=\"1\" uuid=") || text.contains("<!DOCTYPE") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if xmllint_with(&schema(), &name, &text) {
            assert!(xmllint_with(&schema_u2(), &name, &text), "{name} gilt auch nach dem eingefrorenen u2-XSD");
            assert!(xmllint_with(&schema_v1(), &name, &text), "{name} gilt auch nach dem eingefrorenen v1-XSD");
        }
        checked += 1;
    }
    assert!(checked >= 1, "mindestens eine Prüfdatei der Unterversion 1 gefunden");
    // An output without the novelties of sub-version 4 (as Round 27 wrote it) is valid by u4 and u3.
    let u3 = write_xml(&without_u4(sample_document()), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"3\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    if xmllint_with(&schema(), "u3.baruch.xml", &u3) {
        assert!(xmllint_with(&schema_u3(), "u3.baruch.xml", &u3), "dieselbe Datei gilt nach dem u3-XSD");
    }
    // An output without the novelties of sub-version 3 (as Round 25/26 wrote it) is valid by u4 and u2.
    let mut u2 = without_u4(sample_document());
    u2.work.sheet = None;
    let book = u2.work.book.as_mut().unwrap();
    book.cover_blocks.clear();
    u2.work.deleted.retain(|t| t.kind.since_version() <= 2);
    let xml = write_xml(&u2, ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"2\"", 1)
        .replacen(" sprache=\"de\"", "", 1)
        .replace("    <verlag-anschrift>Probeverlag\nAm Brunnen 1\n12345 Musterstadt</verlag-anschrift>\n", "")
        .replace("    <druckerei-anschrift>Probedruck\nDruckgasse 2\n12345 Musterstadt</druckerei-anschrift>\n", "");
    assert!(!xml.contains("anschrift"), "Vorbedingung: eine Datei wie aus Runde 25");
    if xmllint_with(&schema(), "u2.baruch.xml", &xml) {
        assert!(xmllint_with(&schema_u2(), "u2.baruch.xml", &xml), "dieselbe Datei gilt nach dem u2-XSD");
    }
    // And without the novelties of sub-version 2 (as Round 26 wrote it) also by v1.
    let mut old = u2;
    old.work.map = ExchangeMap::default();
    old.work.deleted.retain(|t| t.kind.since_version() == 1);
    let xml = write_xml(&old, ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"1\"", 1)
        .replacen(" sprache=\"de\"", "", 1)
        .replace("    <verlag-anschrift>Probeverlag\nAm Brunnen 1\n12345 Musterstadt</verlag-anschrift>\n", "")
        .replace("    <druckerei-anschrift>Probedruck\nDruckgasse 2\n12345 Musterstadt</druckerei-anschrift>\n", "");
    if xmllint_with(&schema(), "alt.baruch.xml", &xml) {
        assert!(xmllint_with(&schema_v1(), "alt.baruch.xml", &xml), "dieselbe Datei gilt auch nach v1");
    }
}

#[test]
fn the_u3_schema_rejects_broken_cover_blocks() {
    // Cross-check for <umschlag>: unknown kind, position above 1000 mm, foreign colour, font with semicolon, wrong
    // direction, address in the wrong place.
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe des Umschlags übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(
        xml.contains("art=\"vorderseite-titel\"")
            && xml.contains("x-mm=\"15\"")
            && xml.contains("richtung=\"aufwaerts\""),
        "Vorbedingung: {xml}"
    );
    for (what, broken) in [
        ("Art", xml.replace("art=\"vorderseite-titel\"", "art=\"vorderseite-wolke\"")),
        ("Lage", xml.replace("x-mm=\"15\"", "x-mm=\"1000.5\"")),
        ("Farbe", xml.replace("farbe=\"hell\"", "farbe=\"rot\"")),
        ("Schrift", xml.replace("schrift=\"Libertinus Serif\"", "schrift=\"A;B\"")),
        ("Laufrichtung", xml.replace("richtung=\"aufwaerts\"", "richtung=\"seitwaerts\"")),
        ("Reihenfolge", xml.replace("<vita>", "<verlag-anschrift/><vita>")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u3-XSD scheitern");
    }
}

#[test]
fn the_u2_schema_rejects_a_broken_map() {
    // Cross-check for <karte>: an unknown link kind, a coordinate above 1e6 and a foreign element.
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe der Karte übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xml.contains("art=\"fuehrt-weiter\"") && xml.contains("x=\"120.5\""), "Vorbedingung");
    for (what, broken) in [
        ("Art", xml.replace("art=\"fuehrt-weiter\"", "art=\"mag\"")),
        ("Koordinate", xml.replace("x=\"120.5\"", "x=\"1000000.5\"")),
        ("Element", xml.replace("</karte>", "<kante/></karte>")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u3-XSD scheitern");
    }
}

#[test]
fn the_u7_schema_defines_registers_glossary_and_the_new_typeset_attributes() {
    // Round 31 (glossar-daten): <eigene-register>, <glossar> with <begriff>, the four new <satz> attributes and the
    // tombstone kinds glossar-begriff and register; every word the writer outputs is in the schema.
    let u7 = std::fs::read_to_string(schema()).unwrap();
    for needle in [
        "name=\"eigene-register\"",
        "name=\"glossar\"",
        "name=\"begriff\"",
        "name=\"wort\"",
        "name=\"ausfuehrlich\"",
        "name=\"bibelstelle\"",
        "targetNamespace=\"urn:baruch:werk:1\"",
    ] {
        assert!(u7.contains(needle), "u7-XSD enthält {needle}");
    }
    for attribute in baruch_exchange::typeset::TYPESET_ATTRIBUTES {
        assert!(u7.contains(&format!("name=\"{attribute}\"")), "Attribut {attribute} im u7-XSD");
    }
    for kind in ExchangeTombstoneKind::ALL {
        assert!(u7.contains(&format!("value=\"{}\"", kind.as_xml())), "Grabsteinart {} im u7-XSD", kind.as_xml());
    }
    for status in ExchangeGlossaryStatus::ALL {
        assert!(u7.contains(&format!("value=\"{}\"", status.as_xml())), "Status {} im u7-XSD", status.as_xml());
    }
    for word in ["vor-registern", "nach-registern"] {
        assert!(u7.contains(&format!("value=\"{word}\"")), "Glossarseite {word} im u7-XSD");
    }
}

#[test]
fn the_u7_schema_accepts_a_u6_file_and_the_frozen_u6_schema_too_but_not_a_u7_file() {
    // An output without the novelties of sub-version 7 (as Round 29 wrote it) is valid by u7 and by u6.
    let u6 = write_xml(&without_u7(sample_document()), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"6\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    assert!(!u6.contains("<glossar>") && !u6.contains("glossarseite="), "Vorbedingung: ohne u7-Teile");
    if xmllint_with(&schema(), "u6.baruch.xml", &u6) {
        assert!(xmllint_with(&schema_u6(), "u6.baruch.xml", &u6), "dieselbe Datei gilt nach dem u6-XSD");
    }
    // A u7 output with glossary is not valid by u6 (otherwise the schema would not be a real extension).
    let u7 = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xmllint("u7.baruch.xml", &u7) || !xmllint_available(), "u7-Ausgabe besteht das u7-XSD");
    if xmllint_available() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("u7.baruch.xml");
        std::fs::write(&file, &u7).unwrap();
        let output = Command::new("xmllint")
            .args(["--noout", "--nonet", "--schema"])
            .arg(schema_u6())
            .arg(&file)
            .output()
            .unwrap();
        assert!(!output.status.success(), "<glossar> ist im u6-XSD unbekannt");
    }
}

#[test]
fn the_u7_schema_rejects_broken_registers_and_glossary() {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe der Unterversion 7 übersprungen.");
        return;
    }
    let xml = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    assert!(xml.contains("status=\"angenommen\"") && xml.contains("kennung=\"orte\""), "Vorbedingung: {xml}");
    for (what, broken) in [
        ("Status", xml.replacen("status=\"angenommen\"", "status=\"offen\"", 1)),
        ("Kennung", xml.replace("kennung=\"orte\"", "kennung=\"Orte\"")),
        ("Spalten", xml.replace("registerseiten-spalten=\"2\"", "registerseiten-spalten=\"4\"")),
        ("Glossarseite", xml.replace("glossarseite=\"vor-registern\"", "glossarseite=\"hinten\"")),
        ("ohne Wort", xml.replacen("<wort>Brunnenstube</wort>", "", 1)),
        (
            "Reihenfolge",
            xml.replacen("<variante>Brunnenstuben</variante>", "", 1).replacen(
                "<wort>Brunnenstube</wort>",
                "<variante>Brunnenstuben</variante><wort>Brunnenstube</wort>",
                1,
            ),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kaputt.xml");
        std::fs::write(&file, broken).unwrap();
        let output =
            Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema()).arg(&file).output().unwrap();
        assert!(!output.status.success(), "{what}: muss am u7-XSD scheitern");
    }
}

// ---- Sub-version 8 (Round 22): amendments, draft, state on findings ----------------------------------------

/// SHA-256 of the frozen u7 schema (state of development dfff467).
const U7_SHA256: &str = "a4def3e190d8a316be791734e621508bdcaaeeb97a5dce48f15d3dd4d63e9b7e";

/// `Some(true)` if xmllint rejects the file against `schema` (without xmllint: `None`, with a message).
fn rejected_by(schema: &Path, content: &str) -> Option<bool> {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: Gegenprobe übersprungen.");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("probe.xml");
    std::fs::write(&file, content).unwrap();
    let output =
        Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(schema).arg(&file).output().unwrap();
    Some(!output.status.success())
}

#[test]
fn a_u8_output_with_amends_draft_and_stands_passes_the_u8_schema_but_not_the_frozen_u7() {
    // R22 §2.11: every output passes the new XSD; the new parts are unknown to the frozen u7 (real
    // extension), and u7 is byte-unchanged (the_frozen_v1_and_u2_schemas_are_byte_unchanged).
    let doc = baruch_exchange::sample::sample_document_u8();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in ["<fortschreibung ", "stand=\"3\"", "beruht-stand=\"2\"", "<entwurf ", "art=\"fortschreibung\""] {
        assert!(xml.contains(needle), "Vorbedingung: {needle} in der Ausgabe");
    }
    xmllint("u8.baruch.xml", &xml);
    xmllint("u8-dateien.xml", &write_xml(&doc, ImageMode::Files).unwrap());
    if let Some(rejected) = rejected_by(&schema_u7(), &xml) {
        assert!(rejected, "Fortschreibungen und Entwurf sind im u7-XSD unbekannt");
    }
}

#[test]
fn the_u8_schema_accepts_a_u7_file_and_the_frozen_u7_schema_too() {
    // Downward compatible: a file as Round 31 wrote it (without u8 parts, version 7) is valid by u8 and u7.
    let u7 = write_xml(&sample_document(), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"7\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    assert!(!u7.contains("<fortschreibung") && !u7.contains("<entwurf"), "Vorbedingung: ohne u8-Teile");
    if xmllint_with(&schema(), "u7.baruch.xml", &u7) {
        assert!(xmllint_with(&schema_u7(), "u7.baruch.xml", &u7), "dieselbe Datei gilt nach dem u7-XSD");
    }
}

#[test]
fn the_u8_schema_rejects_broken_amends_and_drafts() {
    let xml = write_xml(&baruch_exchange::sample::sample_document_u8(), ImageMode::Inline).unwrap();
    for (what, broken) in [
        ("Art der Fortschreibung", xml.replacen("art=\"undo\"", "art=\"weg\"", 1)),
        ("Schritt 0", xml.replacen("seq=\"1\"", "seq=\"0\"", 1)),
        ("Lage", xml.replacen("lage=\"fusszeile\"", "lage=\"quer\"", 1)),
        ("Deckkraft", xml.replacen("deckkraft=\"20\"", "deckkraft=\"viel\"", 1)),
        ("ohne nachher", xml.replacen("<nachher>", "<danach>", 1).replacen("</nachher>", "</danach>", 1)),
        ("Stand", xml.replacen("stand=\"3\"", "stand=\"-1\"", 1)),
    ] {
        assert_ne!(broken, xml, "{what}: die Ersetzung hat gegriffen");
        if let Some(rejected) = rejected_by(&schema(), &broken) {
            assert!(rejected, "{what}: muss am u8-XSD scheitern");
        }
    }
}

// ---- Sub-version 9 (Round 34): tree of the mind map on the idea --------------------------------------------

/// SHA-256 of the frozen u8 schema (state of development 67faa2f).
const U8_SHA256: &str = "c5fec0876e3914a8a31d43a5a07aeca48e11e464dc8dc120fa09223e7eb71aea";

#[test]
fn a_u9_output_with_the_idea_tree_passes_the_u9_schema_but_not_the_frozen_u8() {
    // Contract R34 §1: the output with titel, eltern, platz and hauptthema passes the new XSD; to the frozen u8
    // the attributes are unknown (real extension), and u8 is byte-unchanged.
    let doc = baruch_exchange::sample::sample_document_u9();
    let xml = write_xml(&doc, ImageMode::Inline).unwrap();
    for needle in ["titel=\"Lampe am Seil\"", " eltern=\"", " platz=\"2\"", " hauptthema=\"true\""] {
        assert!(xml.contains(needle), "Vorbedingung: {needle} in der Ausgabe");
    }
    xmllint("u9.baruch.xml", &xml);
    xmllint("u9-dateien.xml", &write_xml(&doc, ImageMode::Files).unwrap());
    if let Some(rejected) = rejected_by(&schema_u8(), &xml) {
        assert!(rejected, "die Attribute des Baums sind im u8-XSD unbekannt");
    }
}

#[test]
fn the_u9_schema_accepts_a_u8_file_and_the_frozen_u8_schema_too() {
    // Downward compatible: a file as Round 22 wrote it (without tree, version 8) is valid by u9 and u8.
    let u8_file = write_xml(&baruch_exchange::sample::sample_document_u8(), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"8\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    assert!(
        u8_file
            .lines()
            .filter(|l| l.trim_start().starts_with("<einfall "))
            .all(|l| !l.contains(" eltern=") && !l.contains(" titel=")),
        "Vorbedingung: Einfälle ohne Attribute der 9"
    );
    if xmllint_with(&schema(), "u8.baruch.xml", &u8_file) {
        assert!(xmllint_with(&schema_u8(), "u8.baruch.xml", &u8_file), "dieselbe Datei gilt nach dem u8-XSD");
    }
}

#[test]
fn the_u9_schema_rejects_a_long_title_a_broken_parent_and_a_wrong_place() {
    let xml = write_xml(&baruch_exchange::sample::sample_document_u9(), ImageMode::Inline).unwrap();
    assert!(xml.contains("titel=\"Lampe am Seil\"") && xml.contains("platz=\"2\""), "Vorbedingung: {xml}");
    for (what, broken) in [
        (
            "Titel über 200 Zeichen",
            xml.replacen("titel=\"Lampe am Seil\"", &format!("titel=\"{}\"", "a".repeat(201)), 1),
        ),
        ("eltern keine UUID", xml.replacen(" eltern=\"", " eltern=\"x", 1)),
        ("platz keine Zahl", xml.replacen("platz=\"2\"", "platz=\"zwei\"", 1)),
        ("hauptthema kein Wahrheitswert", xml.replacen("hauptthema=\"true\"", "hauptthema=\"ja\"", 1)),
    ] {
        if let Some(rejected) = rejected_by(&schema(), &broken) {
            assert!(rejected, "{what}: muss am u9-XSD scheitern");
        }
    }
}

// ---- Sub-version 10 (Round 38): work language at <angaben> ------------------------------------------------------

/// SHA-256 of the frozen u9 schema (state of development 3fa5806, before Round 38).
const U9_SHA256: &str = "aaf5267929ded5ef6eda9d4e6e522dfbe0bdf8065a102b5d6de9cb0e6decc939";

#[test]
fn a_u10_output_carries_the_work_language_and_passes_the_u10_schema_but_not_the_frozen_u9() {
    assert_eq!(
        sha256_hex(&std::fs::read(schema_u9()).unwrap()),
        U9_SHA256,
        "baruch-werk-1-u9.xsd is frozen: byte for byte unchanged"
    );
    for (doc, code) in
        [(baruch_exchange::sample::sample_document_u9(), "de"), (baruch_exchange::sample::sample_document_u10(), "en")]
    {
        let xml = write_xml(&doc, ImageMode::Inline).unwrap();
        assert!(
            xml.contains(&format!("<angaben sprache=\"{code}\">")),
            "the writer always writes the work language at <angaben>, also for a German work ({code})"
        );
        xmllint(&format!("u10-{code}.baruch.xml"), &xml);
        xmllint(&format!("u10-{code}-dateien.xml"), &write_xml(&doc, ImageMode::Files).unwrap());
        if let Some(rejected) = rejected_by(&schema_u9(), &xml) {
            assert!(rejected, "the attribute sprache is unknown to the frozen u9 schema (a real extension)");
        }
    }
}

#[test]
fn the_u10_schema_accepts_a_u9_file_and_rejects_an_unknown_language() {
    // Downward compatible: a file as Round 34 wrote it (version 9, no attribute) is valid by u10 and by u9.
    let u9_file = write_xml(&baruch_exchange::sample::sample_document_u9(), ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"9\"", 1)
        .replacen(" sprache=\"de\"", "", 1);
    assert!(u9_file.contains("<angaben>"), "precondition: the file carries no work language");
    if xmllint_with(&schema(), "u9.baruch.xml", &u9_file) {
        assert!(
            xmllint_with(&schema_u9(), "u9.baruch.xml", &u9_file),
            "the same file is valid by the frozen u9 schema"
        );
    }
    let xml = write_xml(&baruch_exchange::sample::sample_document_u10(), ImageMode::Inline).unwrap();
    for bad in ["fr", "EN", "", "de en"] {
        let broken = xml.replacen("<angaben sprache=\"en\">", &format!("<angaben sprache=\"{bad}\">"), 1);
        assert_ne!(broken, xml, "precondition: the attribute was replaced");
        if let Some(rejected) = rejected_by(&schema(), &broken) {
            assert!(rejected, "sprache=\"{bad}\" must fail the u10 schema: only de and en are work languages");
        }
    }
}
