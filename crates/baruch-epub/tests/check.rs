// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prüfung des EPUB (Runde 20c, Besitz „epub-pruefung“): eigene Strukturprüfung gegen das gültige Testwerk und je
//! eine gezielt kaputte Variante, Wohlgeformtheit mit xmllint bzw. quick-xml, Warnungen je Reader, Suche und Lauf
//! von EPUBCheck. EPUBCheck ist hier nie das echte Programm, sondern ein selbst geschriebenes Skript in einem
//! Temp-Ordner, übergeben über `path_env` (der PATH des Prozesses bleibt unberührt); nichts wird heruntergeladen.

use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use baruch_epub::check::{
    EpubcheckTool, Finding, Severity, XmlTool, check_structure, codes, find_epubcheck, find_xmllint, reader_warnings,
    run_epubcheck,
};
use baruch_epub::readers::{ReaderProfile, ReaderSupport};
use baruch_epub::{EpubBook, ItemKind, OPF_PATH, build, package, sample};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn book() -> EpubBook {
    build(&sample::input()).expect("Das Testwerk lässt sich bauen")
}

fn good() -> Vec<u8> {
    package(&book()).expect("Das Testwerk lässt sich packen")
}

/// Einträge des ZIP in ihrer Reihenfolge.
fn entries(epub: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(epub)).expect("ZIP");
    (0..zip.len())
        .map(|i| {
            let mut entry = zip.by_index(i).expect("Eintrag");
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("lesen");
            (entry.name().unwrap().into_owned(), bytes)
        })
        .collect()
}

/// ZIP aus `entries`; `mimetype` wird mit `mimetype_method` gespeichert, alles andere DEFLATE.
fn zip_of(entries: &[(String, Vec<u8>)], mimetype_method: CompressionMethod) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        let method = if name == "mimetype" { mimetype_method } else { CompressionMethod::Deflated };
        zip.start_file(name.as_str(), SimpleFileOptions::default().compression_method(method)).expect("Eintrag");
        zip.write_all(bytes).expect("schreiben");
    }
    zip.finish().expect("fertig").into_inner()
}

/// Gültiges Testwerk mit geänderten Einträgen (in derselben Reihenfolge, mimetype STORED).
fn variant(change: impl FnOnce(&mut Vec<(String, Vec<u8>)>)) -> Vec<u8> {
    let mut list = entries(&good());
    change(&mut list);
    zip_of(&list, CompressionMethod::Stored)
}

fn text_mut<'a>(list: &'a mut [(String, Vec<u8>)], name: &str) -> &'a mut Vec<u8> {
    &mut list.iter_mut().find(|(n, _)| n == name).unwrap_or_else(|| panic!("{name} fehlt im Testwerk")).1
}

fn replace(list: &mut [(String, Vec<u8>)], name: &str, from: &str, to: &str) {
    let bytes = text_mut(list, name);
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    assert!(text.contains(from), "„{from}“ fehlt in {name}: {text}");
    *bytes = text.replacen(from, to, 1).into_bytes();
}

/// Erste Kapiteldatei des Testwerks (Pfad im ZIP).
fn first_chapter() -> String {
    let book = book();
    let item = book.items.iter().find(|i| i.kind == ItemKind::Chapter).expect("Das Testwerk hat Kapitel");
    format!("OEBPS/{}", item.href)
}

fn codes_of(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.code.as_str()).collect()
}

fn errors(findings: &[Finding]) -> Vec<&Finding> {
    findings.iter().filter(|f| f.severity == Severity::Error).collect()
}

/// Prüft die Variante mit quick-xml und verlangt den Code als Fehler.
fn assert_error(epub: &[u8], code: &str, why: &str) -> Vec<Finding> {
    let (findings, tool) = check_structure(epub, None);
    assert_eq!(tool, XmlTool::QuickXml, "ohne xmllint prüft quick-xml");
    assert!(
        findings.iter().any(|f| f.code == code && f.severity == Severity::Error),
        "{why}: Fehler {code} erwartet, gefunden {:?}",
        codes_of(&findings)
    );
    findings
}

#[test]
fn the_valid_sample_has_no_error() {
    let (findings, tool) = check_structure(&good(), None);
    println!("Befunde des Testwerks: {:?}", codes_of(&findings));
    assert_eq!(tool, XmlTool::QuickXml, "ohne xmllint nennt der Bericht quick-xml");
    assert!(errors(&findings).is_empty(), "ein gebautes und gepacktes Testwerk ist gültig: {findings:#?}");
    assert!(
        findings.iter().all(|f| f.reader.is_none()),
        "die Strukturprüfung meldet nichts je Reader (das macht reader_warnings)"
    );
}

#[test]
fn a_mimetype_that_is_not_the_first_entry_is_an_error() {
    let mut list = entries(&good());
    let mimetype = list.remove(0);
    list.insert(1, mimetype);
    assert_error(
        &zip_of(&list, CompressionMethod::Stored),
        codes::MIMETYPE_NOT_FIRST,
        "Reader erkennen EPUB am ersten Eintrag",
    );
}

#[test]
fn a_compressed_mimetype_is_an_error() {
    let epub = zip_of(&entries(&good()), CompressionMethod::Deflated);
    assert_error(&epub, codes::MIMETYPE_COMPRESSED, "OCF verlangt mimetype unkomprimiert");
}

#[test]
fn a_missing_container_is_an_error() {
    let epub = variant(|list| list.retain(|(n, _)| n != "META-INF/container.xml"));
    assert_error(&epub, codes::CONTAINER_MISSING, "ohne container.xml findet kein Reader das OPF");
}

#[test]
fn a_manifest_entry_without_its_file_is_an_error() {
    let chapter = first_chapter();
    let epub = variant(|list| list.retain(|(n, _)| *n != chapter));
    let findings = assert_error(&epub, codes::MANIFEST_FILE_MISSING, "das Manifest nennt eine Datei, die fehlt");
    assert!(
        findings.iter().any(|f| f.code == codes::MANIFEST_FILE_MISSING && f.path.as_deref() == Some(chapter.as_str())),
        "der Befund nennt die fehlende Datei: {findings:#?}"
    );
}

#[test]
fn a_file_without_manifest_entry_is_an_error() {
    let epub = variant(|list| list.push(("OEBPS/text/vergessen.xhtml".into(), b"<html/>".to_vec())));
    let findings = assert_error(&epub, codes::FILE_NOT_IN_MANIFEST, "jede Datei des Pakets gehört ins Manifest");
    assert!(
        findings.iter().any(|f| f.path.as_deref() == Some("OEBPS/text/vergessen.xhtml")),
        "der Befund nennt die überzählige Datei"
    );
}

#[test]
fn an_unknown_spine_id_is_an_error() {
    let epub = variant(|list| replace(list, OPF_PATH, "</spine>", "<itemref idref=\"gibt-es-nicht\"/>\n</spine>"));
    assert_error(&epub, codes::SPINE_IDREF_UNKNOWN, "die Lesereihenfolge nennt eine unbekannte id");
}

#[test]
fn a_not_well_formed_xhtml_file_is_an_error() {
    let chapter = first_chapter();
    let epub = variant(|list| replace(list, &chapter, "</section>", "<p>offen</section>"));
    let findings = assert_error(&epub, codes::XHTML_NOT_WELLFORMED, "ein offenes <p> macht das XHTML kaputt");
    assert!(
        findings.iter().any(|f| f.code == codes::XHTML_NOT_WELLFORMED && f.path.as_deref() == Some(chapter.as_str())),
        "der Befund nennt die kaputte Datei"
    );
}

#[test]
fn a_missing_navigation_document_is_an_error() {
    let epub = variant(|list| replace(list, OPF_PATH, " properties=\"nav\"", ""));
    assert_error(&epub, codes::NAV_MISSING, "EPUB 3 verlangt ein Navigationsdokument");
}

#[test]
fn a_missing_language_is_an_error() {
    let epub = variant(|list| {
        let bytes = text_mut(list, OPF_PATH);
        let text = String::from_utf8(bytes.clone()).expect("UTF-8");
        let start = text.find("<dc:language>").expect("Sprache im Testwerk");
        let end = text[start..].find("</dc:language>").expect("Ende") + start + "</dc:language>".len();
        *bytes = format!("{}{}", &text[..start], &text[end..]).into_bytes();
    });
    assert_error(&epub, codes::LANGUAGE_MISSING, "ohne dc:language raten Reader Silbentrennung und Aussprache");
}

/// Testwerk mit einem zusätzlichen Bild in der ersten Kapiteldatei (`img` wie angegeben).
fn with_image(img: &str) -> Vec<u8> {
    let chapter = first_chapter();
    let img = img.to_owned();
    variant(move |list| {
        replace(list, &chapter, "</section>", &format!("<p>{img}</p>\n</section>"));
        replace(
            list,
            OPF_PATH,
            "</manifest>",
            "<item id=\"probe\" href=\"images/probe.png\" media-type=\"image/png\"/>\n</manifest>",
        );
        list.push(("OEBPS/images/probe.png".into(), sample::png(2, 2, [10, 20, 30])));
    })
}

#[test]
fn an_image_without_alt_is_an_error_and_an_empty_alt_only_a_hint() {
    let missing = with_image("<img src=\"../images/probe.png\"/>");
    assert_error(&missing, codes::IMG_ALT_MISSING, "ohne alt liest ein Vorleseprogramm den Dateinamen");
    let (findings, _) = check_structure(&with_image("<img src=\"../images/probe.png\" alt=\"\"/>"), None);
    assert!(errors(&findings).is_empty(), "alt=\"\" ist erlaubt (Schmuckbild): {findings:#?}");
    // Seit R20a bringt das Testwerk selbst ein Schmuckbild (alt="") mit; gezählt wird darum gegen die Ausgangslage.
    let alt_findings = |list: &[Finding], code: &str| list.iter().filter(|f| f.code == code).count();
    let (base, _) = check_structure(&good(), None);
    assert!(
        alt_findings(&findings, codes::IMG_ALT_EMPTY) == alt_findings(&base, codes::IMG_ALT_EMPTY) + 1
            && findings.iter().filter(|f| f.code == codes::IMG_ALT_EMPTY).all(|f| f.severity == Severity::Info),
        "ein leerer Alternativtext ist ein Hinweis zur Barrierefreiheit: {:?}",
        codes_of(&findings)
    );
    let (described, _) = check_structure(&with_image("<img src=\"../images/probe.png\" alt=\"Probe\"/>"), None);
    assert!(
        alt_findings(&described, codes::IMG_ALT_EMPTY) == alt_findings(&base, codes::IMG_ALT_EMPTY)
            && alt_findings(&described, codes::IMG_ALT_MISSING) == 0,
        "ein beschriebenes Bild ist in Ordnung: {:?}",
        codes_of(&described)
    );
}

#[test]
fn a_reference_to_a_missing_file_is_an_error() {
    let chapter = first_chapter();
    let epub = variant(|list| replace(list, &chapter, "</section>", "<p><a href=\"weg.xhtml\">weg</a></p></section>"));
    assert_error(&epub, codes::REFERENCE_MISSING, "ein Verweis auf eine fehlende Datei führt ins Leere");
}

#[test]
fn a_table_of_contents_against_the_spine_order_is_reported() {
    let book = book();
    let chapters: Vec<String> =
        book.items.iter().filter(|i| i.kind == ItemKind::Chapter).map(|i| i.id.clone()).collect();
    assert!(chapters.len() >= 2, "das Testwerk hat mindestens zwei Kapitel");
    let (a, b) = (&chapters[0], &chapters[1]);
    // Spine umdrehen: die beiden Kapitel tauschen die Plätze, das Inhaltsverzeichnis bleibt.
    let epub = variant(|list| {
        replace(list, OPF_PATH, &format!("<itemref idref=\"{a}\"/>"), "<itemref idref=\"PLATZHALTER\"/>");
        replace(list, OPF_PATH, &format!("<itemref idref=\"{b}\"/>"), &format!("<itemref idref=\"{a}\"/>"));
        replace(list, OPF_PATH, "<itemref idref=\"PLATZHALTER\"/>", &format!("<itemref idref=\"{b}\"/>"));
    });
    let (findings, _) = check_structure(&epub, None);
    assert!(
        findings.iter().any(|f| f.code == codes::SPINE_NAV_ORDER),
        "Lesereihenfolge und Inhaltsverzeichnis widersprechen sich: {:?}",
        codes_of(&findings)
    );
}

#[test]
fn something_that_is_no_zip_is_reported_and_not_a_panic() {
    let (findings, _) = check_structure(b"kein zip", None);
    assert_eq!(codes_of(&findings), vec![codes::ZIP_INVALID], "nur ein Befund: kein lesbares ZIP");
}

#[test]
fn well_formedness_uses_xmllint_when_it_is_found() {
    let Some(xmllint) = find_xmllint(std::env::var_os("PATH").as_deref()) else {
        println!("ÜBERSPRUNGEN: xmllint nicht im PATH (Rückfall quick-xml prüft der Rest dieser Datei)");
        return;
    };
    let (findings, tool) = check_structure(&good(), Some(&xmllint));
    assert_eq!(tool, XmlTool::Xmllint, "mit gefundenem xmllint nennt der Bericht xmllint");
    assert!(errors(&findings).is_empty(), "xmllint findet das Testwerk wohlgeformt: {findings:#?}");
    let chapter = first_chapter();
    let broken = variant(|list| replace(list, &chapter, "</section>", "<p>offen</section>"));
    let (findings, tool) = check_structure(&broken, Some(&xmllint));
    assert_eq!(tool, XmlTool::Xmllint, "auch beim Fehler bleibt xmllint das Werkzeug");
    let found = findings.iter().find(|f| f.code == codes::XHTML_NOT_WELLFORMED).expect("xmllint meldet das offene <p>");
    assert!(found.message.contains("xmllint"), "die Meldung nennt xmllint: {}", found.message);
}

#[test]
fn an_xmllint_that_does_not_run_falls_back_to_quick_xml() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("xmllint");
    let (findings, tool) = check_structure(&good(), Some(&missing));
    assert_eq!(tool, XmlTool::QuickXml, "lässt sich xmllint nicht starten, prüft quick-xml – und der Bericht sagt es");
    assert!(errors(&findings).is_empty(), "der Rückfall prüft genauso: {findings:#?}");
}

#[test]
fn xmllint_is_found_only_as_an_executable_in_the_given_path() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(find_xmllint(Some(dir.path().as_os_str())), None, "leerer Ordner");
    let plain = dir.path().join("xmllint");
    std::fs::write(&plain, "#!/bin/sh\n").unwrap();
    assert_eq!(find_xmllint(Some(dir.path().as_os_str())), None, "nicht ausführbar zählt nicht");
    make_executable(&plain);
    assert_eq!(find_xmllint(Some(dir.path().as_os_str())), Some(plain), "ausführbar im übergebenen Ordner");
    assert_eq!(find_xmllint(None), None, "ohne PATH kein xmllint");
}

// ---- Reader ----------------------------------------------------------------------------------------

const NOTHING: ReaderSupport = ReaderSupport {
    popup_footnotes: false,
    embedded_fonts: false,
    css_backgrounds: false,
    page_break_css: false,
    hyphenation: false,
    svg_images: false,
    night_mode: false,
    overrides_publisher_font: false,
};

fn profile(slug: &'static str, supports: ReaderSupport) -> ReaderProfile {
    ReaderProfile {
        slug,
        name: "Probe-Reader",
        device: "Probe",
        width_px: 300,
        height_px: 400,
        required: false,
        via_conversion: false,
        supports,
        note: "",
    }
}

/// Testwerk mit einer Popup-Fußnote in der ersten Kapiteldatei.
fn book_with_popup() -> EpubBook {
    let mut book = book();
    let chapter = book.items.iter_mut().find(|i| i.kind == ItemKind::Chapter).expect("Kapitel");
    let text = String::from_utf8(chapter.bytes.clone()).unwrap();
    if !text.contains("epub:type=\"noteref\"") {
        chapter.bytes = text
            .replacen(
                "</section>",
                "<p>Text<a epub:type=\"noteref\" href=\"#fn1\">1</a></p><aside epub:type=\"footnote\" id=\"fn1\">\
                 Notiz</aside></section>",
                1,
            )
            .into_bytes();
    }
    book
}

#[test]
fn a_reader_without_popup_footnotes_gets_a_hint_when_the_book_has_them() {
    let warnings = reader_warnings(&book_with_popup(), &profile("probe", NOTHING));
    let popup = warnings.iter().find(|f| f.code == codes::READER_POPUP_FOOTNOTES).expect("Hinweis zu Popup-Fußnoten");
    assert!(popup.message.contains("Fußnoten"), "die Meldung erklärt es: {}", popup.message);
    let able =
        reader_warnings(&book_with_popup(), &profile("probe", ReaderSupport { popup_footnotes: true, ..NOTHING }));
    assert!(
        !able.iter().any(|f| f.code == codes::READER_POPUP_FOOTNOTES),
        "ein Reader mit Popups bekommt keinen Hinweis"
    );
}

#[test]
fn a_reader_without_page_break_css_gets_a_hint_about_chapter_breaks() {
    let warnings = reader_warnings(&book(), &profile("probe", NOTHING));
    let breaks = warnings.iter().find(|f| f.code == codes::READER_PAGE_BREAKS).expect("Hinweis zu Kapitelumbrüchen");
    assert!(breaks.message.contains("Kapitel"), "die Meldung spricht von Kapiteln: {}", breaks.message);
    let able = reader_warnings(&book(), &profile("probe", ReaderSupport { page_break_css: true, ..NOTHING }));
    assert!(
        !able.iter().any(|f| f.code == codes::READER_PAGE_BREAKS),
        "wer break-before kennt, braucht keinen Hinweis"
    );
}

#[test]
fn the_publisher_font_hint_needs_a_font_the_book_actually_sets() {
    let reader = profile("probe", ReaderSupport { overrides_publisher_font: true, ..NOTHING });
    // Ohne Satz-Einstellung keine Schrift: der Grundstil (monospace für fehlende Bilder) zählt nicht.
    let plain = reader_warnings(&book(), &reader);
    assert!(
        !plain.iter().any(|f| f.code == codes::READER_PUBLISHER_FONT),
        "ohne eingestellte Schrift kein Hinweis auf die Schrift des Verlags: {plain:?}"
    );
    let mut input = sample::input();
    input.typeset.font = Some("Probe Serif".into());
    let with_font = reader_warnings(&build(&input).unwrap(), &reader);
    assert!(
        with_font.iter().any(|f| f.code == codes::READER_PUBLISHER_FONT),
        "mit eingestellter Schrift der Hinweis: {with_font:?}"
    );
}

#[test]
fn every_reader_warning_names_its_reader() {
    let mut book = book_with_popup();
    book.items.push(baruch_epub::EpubItem {
        id: "svg".into(),
        href: "images/a.svg".into(),
        media_type: "image/svg+xml".into(),
        kind: ItemKind::Image,
        properties: None,
        title: None,
        key: None,
        bytes: b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
    });
    let warnings = reader_warnings(&book, &profile("probe-slug", NOTHING));
    assert!(warnings.iter().any(|f| f.code == codes::READER_SVG), "SVG ohne SVG-Unterstützung wird gemeldet");
    assert!(warnings.len() >= 3, "Popup, Umbrüche und SVG: {warnings:#?}");
    for w in &warnings {
        assert_eq!(w.reader.as_deref(), Some("probe-slug"), "jede Warnung trägt den Slug: {w:?}");
        assert!(w.message.starts_with("Probe-Reader"), "die Meldung nennt den Reader: {}", w.message);
    }
}

// ---- EPUBCheck -------------------------------------------------------------------------------------

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn script(dir: &Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("epubcheck");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    make_executable(&path);
    path
}

#[test]
fn a_script_in_the_given_path_comes_before_a_configured_jar() {
    let bin = tempfile::tempdir().unwrap();
    let jar_dir = tempfile::tempdir().unwrap();
    let jar = jar_dir.path().join("epubcheck.jar");
    std::fs::write(&jar, b"PK").unwrap();
    let path = script(bin.path(), "exit 0");
    assert_eq!(
        find_epubcheck(Some(bin.path().as_os_str()), Some(&jar)),
        Some(EpubcheckTool::Binary { path }),
        "epubcheck im PATH geht vor dem Jar"
    );
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        find_epubcheck(Some(empty.path().as_os_str()), Some(&jar)),
        Some(EpubcheckTool::Jar { path: jar.clone() }),
        "ohne Programm im PATH das eingestellte Jar"
    );
    let gone = jar_dir.path().join("weg.jar");
    assert_eq!(
        find_epubcheck(Some(empty.path().as_os_str()), Some(&gone)),
        None,
        "ein Jar, das es nicht gibt, zählt nicht"
    );
    let zip = jar_dir.path().join("epubcheck.zip");
    std::fs::write(&zip, b"PK").unwrap();
    assert_eq!(find_epubcheck(Some(empty.path().as_os_str()), Some(&zip)), None, "nur Dateien mit .jar");
    assert_eq!(find_epubcheck(None, None), None, "ohne beides: nicht gefunden");
}

fn epub_file(dir: &Path) -> std::path::PathBuf {
    let file = dir.join("buch.epub");
    std::fs::write(&file, good()).unwrap();
    file
}

#[test]
fn run_epubcheck_reads_messages_version_and_exit_code_of_the_script() {
    let dir = tempfile::tempdir().unwrap();
    let file = epub_file(dir.path());
    let failing = script(
        dir.path(),
        "echo 'EPUBCheck v5.1.0'\necho \"ERROR(RSC-005): $1/OEBPS/text/k11.xhtml(3,4): Fehler beim Lesen\" >&2\n\
         echo \"WARNING(OPF-085): $1: Kennung\" >&2\nexit 1",
    );
    let (findings, version, ok) =
        run_epubcheck(&EpubcheckTool::Binary { path: failing }, &file, Duration::from_secs(20)).expect("Lauf");
    assert!(!ok, "Exit 1 mit Fehlern heißt: nicht bestanden");
    assert_eq!(version.as_deref(), Some("5.1.0"), "Version aus der Ausgabe");
    let error = findings.iter().find(|f| f.severity == Severity::Error).expect("ERROR wird ein Fehler");
    assert_eq!(error.code, "EPUBCHECK_RSC-005", "Code mit Vorsilbe EPUBCHECK_");
    assert_eq!(error.path.as_deref(), Some("OEBPS/text/k11.xhtml(3,4)"), "Pfad im EPUB statt Temp-Pfad");
    assert!(findings.iter().any(|f| f.severity == Severity::Warning), "WARNING wird eine Warnung");

    let dir2 = tempfile::tempdir().unwrap();
    let file2 = epub_file(dir2.path());
    let passing = script(dir2.path(), "echo 'EPUBCheck v5.1.0'\necho 'No errors or warnings detected.'\nexit 0");
    let (findings, _, ok) =
        run_epubcheck(&EpubcheckTool::Binary { path: passing }, &file2, Duration::from_secs(20)).expect("Lauf");
    assert!(ok && findings.is_empty(), "Exit 0 ohne Meldungen heißt: bestanden ({findings:?})");
}

#[test]
fn a_hanging_epubcheck_is_stopped_at_the_time_limit() {
    let dir = tempfile::tempdir().unwrap();
    let file = epub_file(dir.path());
    let hanging = script(dir.path(), "sleep 30\necho fertig");
    let started = Instant::now();
    let result = run_epubcheck(&EpubcheckTool::Binary { path: hanging }, &file, Duration::from_millis(400));
    let error = result.expect_err("ein hängender Lauf endet mit Fehler");
    assert!(error.contains("abgebrochen"), "die Meldung nennt den Abbruch: {error}");
    assert!(started.elapsed() < Duration::from_secs(10), "nicht auf das Skript gewartet: {:?}", started.elapsed());
}
