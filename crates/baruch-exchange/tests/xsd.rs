// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26 „export“: Die Ausgabe besteht das XSD; seit Runde 22 (Unterversion 8) das XSD
//! `docs/schema/baruch-werk-1-u8.xsd`; `baruch-werk-1-u7.xsd` (Unterversion 7), `baruch-werk-1-u6.xsd` (Unterversion 6), `baruch-werk-1-u5.xsd` (Unterversion 5), `baruch-werk-1-u4.xsd` (Unterversion 4),
//! `baruch-werk-1-u3.xsd` (Unterversion 3), `baruch-werk-1-u2.xsd` (Unterversion 2) und `baruch-werk-1.xsd`
//! (Unterversion 1) bleiben eingefroren (byte-gleich, geprüft über SHA-256).
//!
//! Geprüft wird mit `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u8.xsd DATEI` (Vertrag F2). Fehlt
//! xmllint, meldet der Test das ehrlich und überspringt nur diesen Teil; die Rust-Prüfung der Pflichtelemente über
//! quick-xml-Ereignisse (nicht über den Leser von import) läuft immer.

use std::path::{Path, PathBuf};
use std::process::Command;

use baruch_exchange::sample::sample_document;
use baruch_exchange::write::{ImageMode, write_baruch, write_manifest, write_xml};
use baruch_exchange::*;
use quick_xml::events::Event;

/// Schema der Unterversion 8 (Runde 22), gegen das jede Ausgabe geprüft wird.
fn schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u8.xsd")
}

/// Eingefrorenes Schema der Unterversion 7 (Runde 31).
fn schema_u7() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u7.xsd")
}

/// Eingefrorenes Schema der Unterversion 6 (Runde 29).
fn schema_u6() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u6.xsd")
}

/// Runde 31: das Beispiel ohne die Teile der Unterversion 7 (eigene Register, Glossar, Register/Glossar-Felder der
/// Satz-Einstellungen, Grabsteinarten `glossar-begriff` und `register`).
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

/// Eingefrorenes Schema der Unterversion 5 (Runde 30).
fn schema_u5() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u5.xsd")
}

/// Runde 29: das Beispiel ohne die Teile der Unterversion 6 (Satz-Einstellungen, Grabsteinart `satz`).
fn without_u6(doc: ExchangeDocument) -> ExchangeDocument {
    let mut doc = without_u7(doc);
    doc.work.typeset = None;
    doc.work.deleted.retain(|t| t.kind.since_version() <= 5);
    doc
}

/// Eingefrorenes Schema der Unterversion 4 (Runde 28).
fn schema_u4() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u4.xsd")
}

/// Runde 30: das Beispiel ohne die Teile der Unterversion 5 (Lösungen, Varianten, Schlüssel, Beitrags-Metadaten).
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

/// Eingefrorenes Schema der Unterversion 3 (Runde 27).
fn schema_u3() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u3.xsd")
}

/// Das Beispiel ohne die Teile der Unterversion 4 (Befunde, Bearbeitungen von Beiträgen, Grabsteine ab 4).
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

/// Eingefrorenes Schema der Unterversion 2 (Runde 25).
fn schema_u2() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u2.xsd")
}

/// Eingefrorenes Schema der Unterversion 1.
fn schema_v1() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1.xsd")
}

fn xmllint_available() -> bool {
    Command::new("xmllint").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Prüft eine Datei mit xmllint gegen das XSD; `false`, wenn xmllint fehlt (dann mit Meldung).
fn xmllint(name: &str, content: &str) -> bool {
    xmllint_with(&schema(), name, content)
}

/// Wie [`xmllint`], gegen ein bestimmtes Schema.
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

/// Rust-Prüfung der Pflichtelemente: Wurzel im Namensraum, Kinder in der Reihenfolge des Vokabulars,
/// je Punkt Titel und mindestens eine Fassung, je Fassung genau ein `<text>`, je Bildfassung genau eines von
/// `datei` und `<daten>`.
fn check_required(xml: &str, images: ImageMode) {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    // Je offenes Element: Name, Namen der Kinder, ob `datei` gesetzt ist.
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
                let name = String::from_utf8(e.local_name().as_ref().to_vec()).unwrap();
                if !root_seen {
                    root_seen = true;
                    assert_eq!(name, "werk", "Wurzel ist <werk>");
                    let ns = e.try_get_attribute("xmlns").unwrap().expect("xmlns an der Wurzel");
                    assert_eq!(ns.unescape_value().unwrap(), NAMESPACE, "Namensraum");
                    let version = e.try_get_attribute("version").unwrap().expect("version an der Wurzel");
                    assert_eq!(version.unescape_value().unwrap(), "8", "Unterversion 8 (Runde 22)");
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
    // Runde 31: hinter <befunde> die eigenen Register und das Glossar (neue Elemente ans Ende der Folge, §6).
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
    // Unterversion 3: <satzbild> am Ende von <angaben>, seit Unterversion 6 dahinter <satz> (neue Elemente ans Ende
    // der Folge, §6).
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
    // Erst alle Verbindungen, dann alle Lagen (xs:sequence im XSD der Unterversion 2).
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
    // Das Manifest aus einem echten Container (mit Bildern), nicht von Hand gebaut.
    let bytes = write_baruch(&sample_document()).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut manifest = String::new();
    std::io::Read::read_to_string(&mut archive.by_name(ENTRY_MANIFEST).unwrap(), &mut manifest).unwrap();
    assert!(manifest.contains(&format!("<manifest xmlns=\"{NAMESPACE}\" version=\"8\">")), "Wurzel im Namensraum");
    xmllint("MANIFEST.xml", &manifest);
    // Auch write_manifest allein.
    let entries = [ExchangeManifestEntry { path: ENTRY_WORK.into(), sha256: sha256_hex(b"x"), bytes: 1 }];
    xmllint("einzeln.xml", &write_manifest(&entries).unwrap());
}

#[test]
fn schema_rejects_a_broken_file_so_the_check_has_teeth() {
    // Gegenprobe: Ohne sie bliebe unbemerkt, wenn xmllint gar nicht gegen das Schema prüfte.
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
    // Vertrag R27 1.11: u2 und v1 bleiben eingefroren. Die Prüfsummen sind die Stände von development vor Runde 27
    // (af07921/cd54cbb); jede Änderung an den Dateien fiele hier auf. Seit Runde 28 ist auch u3 eingefroren (Stand
    // von development cb7b169).
    let sha = |path: PathBuf| sha256_hex(&std::fs::read(path).unwrap());
    // Runde 22: auch u7 ist eingefroren (Stand von development dfff467).
    assert_eq!(sha(schema_u7()), U7_SHA256, "baruch-werk-1-u7.xsd byte-unverändert");
    // Runde 31: auch u6 ist eingefroren (Stand von development 5f97a8b).
    assert_eq!(
        sha(schema_u6()),
        "bab4657a6a971c129c5e4685d2bd8143f41aa18ed5c4cd7ef54ced807cf26d38",
        "baruch-werk-1-u6.xsd byte-unverändert"
    );
    // Runde 29: auch u5 ist eingefroren (Stand von development d3a2a8f).
    assert_eq!(
        sha(schema_u5()),
        "106d3a87dbc84351c65c1d1b2496f164ef8037d65c9f2270a9b9a8172627fe17",
        "baruch-werk-1-u5.xsd byte-unverändert"
    );
    // Runde 30: auch u4 ist eingefroren (Stand von development aa6808c).
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
    // Jedes XML-Wort des Modells steht im Schema (sonst bestünde eine Ausgabe das XSD nicht).
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
    // Jedes XML-Wort des Modells steht im Schema (sonst bestünde eine Ausgabe das XSD nicht).
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
    // Vertrag R29 §6.6: <satz> in <angaben> mit jedem Attribut, das der Schreiber kennt, und die Grabsteinart satz.
    // Seit Runde 31 ist u6 eingefroren; geprüft werden die Teile bis Unterversion 6.
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
    // Jedes Wort jeder Aufzählung, das der Schreiber ausgibt, steht im Schema.
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
    // Vertrag R29 §6.6: Eine Ausgabe ohne die Neuerungen der Unterversion 6 (wie sie Runde 30 schrieb) gilt nach u6
    // und nach dem eingefrorenen u5.
    let u5 = write_xml(&without_u6(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"5\"",
        1,
    );
    assert!(!u5.contains("<satz "), "Vorbedingung: ohne <satz>");
    if xmllint_with(&schema(), "u5.baruch.xml", &u5) {
        assert!(xmllint_with(&schema_u5(), "u5.baruch.xml", &u5), "dieselbe Datei gilt nach dem u5-XSD");
    }
    // Eine u6-Ausgabe mit <satz> gilt dagegen nicht nach u5 (sonst wäre das Schema keine echte Erweiterung).
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
    // Eine Ausgabe ohne die Neuerungen der Unterversion 5 (wie sie Runde 28 schrieb) gilt nach u5 und u4.
    let u4 = write_xml(&without_u5(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"4\"",
        1,
    );
    if xmllint_with(&schema(), "u4.baruch.xml", &u4) {
        assert!(xmllint_with(&schema_u4(), "u4.baruch.xml", &u4), "dieselbe Datei gilt nach dem u4-XSD");
    }
}

#[test]
fn the_u4_schema_rejects_broken_findings() {
    // Gegenprobe für <befunde>: unbekannte Art, unbekannter Status, fremdes Kind, Persona leer, Entscheidung fremd.
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
        // <bezug> nach <alte-fassung>: die alten Fassungen stehen am Ende des Beitrags.
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
    // Unterversion 1 bleibt eingefroren: lax geprüftes <karte>, Grabsteinarten ohne verbindung/lage.
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
    // Die handgeschriebene Prüfdatei der Unterversion 1 (die übrigen Dateien unter tests/data/import sind
    // absichtlich unsicher – DTD, Entitäten – und gelten in keiner Unterversion).
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
    // Eine Ausgabe ohne die Neuerungen der Unterversion 4 (wie sie Runde 27 schrieb) gilt nach u4 und u3.
    let u3 = write_xml(&without_u4(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"3\"",
        1,
    );
    if xmllint_with(&schema(), "u3.baruch.xml", &u3) {
        assert!(xmllint_with(&schema_u3(), "u3.baruch.xml", &u3), "dieselbe Datei gilt nach dem u3-XSD");
    }
    // Eine Ausgabe ohne die Neuerungen der Unterversion 3 (wie sie Runde 25/26 schrieb) gilt nach u4 und u2.
    let mut u2 = without_u4(sample_document());
    u2.work.sheet = None;
    let book = u2.work.book.as_mut().unwrap();
    book.cover_blocks.clear();
    u2.work.deleted.retain(|t| t.kind.since_version() <= 2);
    let xml = write_xml(&u2, ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"2\"", 1)
        .replace("    <verlag-anschrift>Probeverlag\nAm Brunnen 1\n12345 Musterstadt</verlag-anschrift>\n", "")
        .replace("    <druckerei-anschrift>Probedruck\nDruckgasse 2\n12345 Musterstadt</druckerei-anschrift>\n", "");
    assert!(!xml.contains("anschrift"), "Vorbedingung: eine Datei wie aus Runde 25");
    if xmllint_with(&schema(), "u2.baruch.xml", &xml) {
        assert!(xmllint_with(&schema_u2(), "u2.baruch.xml", &xml), "dieselbe Datei gilt nach dem u2-XSD");
    }
    // Und ohne die Neuerungen der Unterversion 2 (wie sie Runde 26 schrieb) auch nach v1.
    let mut old = u2;
    old.work.map = ExchangeMap::default();
    old.work.deleted.retain(|t| t.kind.since_version() == 1);
    let xml = write_xml(&old, ImageMode::Inline)
        .unwrap()
        .replacen(&format!("version=\"{FORMAT_VERSION}\""), "version=\"1\"", 1)
        .replace("    <verlag-anschrift>Probeverlag\nAm Brunnen 1\n12345 Musterstadt</verlag-anschrift>\n", "")
        .replace("    <druckerei-anschrift>Probedruck\nDruckgasse 2\n12345 Musterstadt</druckerei-anschrift>\n", "");
    if xmllint_with(&schema(), "alt.baruch.xml", &xml) {
        assert!(xmllint_with(&schema_v1(), "alt.baruch.xml", &xml), "dieselbe Datei gilt auch nach v1");
    }
}

#[test]
fn the_u3_schema_rejects_broken_cover_blocks() {
    // Gegenprobe für <umschlag>: unbekannte Art, Lage über 1000 mm, fremde Farbe, Schrift mit Semikolon, falsche
    // Laufrichtung, Anschrift an falscher Stelle.
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
    // Gegenprobe für <karte>: eine unbekannte Verbindungsart, eine Koordinate über 1e6 und ein fremdes Element.
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
    // Runde 31 (glossar-daten): <eigene-register>, <glossar> mit <begriff>, die vier neuen <satz>-Attribute und die
    // Grabsteinarten glossar-begriff und register; jedes Wort, das der Schreiber ausgibt, steht im Schema.
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
    // Eine Ausgabe ohne die Neuerungen der Unterversion 7 (wie sie Runde 29 schrieb) gilt nach u7 und nach u6.
    let u6 = write_xml(&without_u7(sample_document()), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"6\"",
        1,
    );
    assert!(!u6.contains("<glossar>") && !u6.contains("glossarseite="), "Vorbedingung: ohne u7-Teile");
    if xmllint_with(&schema(), "u6.baruch.xml", &u6) {
        assert!(xmllint_with(&schema_u6(), "u6.baruch.xml", &u6), "dieselbe Datei gilt nach dem u6-XSD");
    }
    // Eine u7-Ausgabe mit Glossar gilt nicht nach u6 (sonst wäre das Schema keine echte Erweiterung).
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

// ---- Unterversion 8 (Runde 22): Fortschreibungen, Entwurf, Stand an Befunden -------------------------------

/// SHA-256 des eingefrorenen u7-Schemas (Stand von development dfff467).
const U7_SHA256: &str = "a4def3e190d8a316be791734e621508bdcaaeeb97a5dce48f15d3dd4d63e9b7e";

/// `Some(true)`, wenn xmllint die Datei gegen `schema` ablehnt (ohne xmllint: `None`, mit Meldung).
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
    // R22 §2.11: jede Ausgabe besteht das neue XSD; die neuen Teile sind dem eingefrorenen u7 unbekannt (echte
    // Erweiterung), und u7 ist byte-unverändert (the_frozen_v1_and_u2_schemas_are_byte_unchanged).
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
    // Abwärtsverträglich: eine Datei, wie Runde 31 sie schrieb (ohne u8-Teile, version 7), gilt nach u8 und u7.
    let u7 = write_xml(&sample_document(), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"7\"",
        1,
    );
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
