// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_render::cli` (Runde 36, Typst-Kommandozeile): eine Datei von der Platte setzen – PDF, PNG, SVG,
//! Wurzelordner als Grenze, Schriftordner, Meldungen mit Ort, keine Pakete. Alle Texte frei erfunden.

use std::path::{Path, PathBuf};

use baruch_render::cli::{CompileJob, OutputFormat, TYPST_VERSION, compile_file, font_names};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Zwei Seiten A6 über den Fährmann am Steinsee.
const FERRY: &str = "#set page(width: 105mm, height: 148mm)\n= Der Fährmann am Steinsee\n\nJonte ruderte jeden Morgen \
                     zur Insel hinüber.\n#pagebreak()\nAm Abend brachte er die Laternen zurück.\n";

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, text).unwrap();
    path
}

fn job(input: PathBuf, format: OutputFormat) -> CompileJob {
    CompileJob { input, root: None, font_paths: Vec::new(), format, ppi: 144.0 }
}

/// Breite eines PNG in Pixeln (IHDR, Bytes 16–19).
fn png_width(png: &[u8]) -> u32 {
    u32::from_be_bytes(png[16..20].try_into().unwrap())
}

#[test]
fn a_file_is_set_to_pdf_png_and_svg() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "faehrmann.typ", FERRY);

    let pdf = compile_file(&job(input.clone(), OutputFormat::Pdf)).expect("PDF");
    assert_eq!(pdf.pages.len(), 1, "ein PDF ist eine Datei, gleich wie viele Seiten es hat");
    assert!(pdf.pages[0].starts_with(b"%PDF-"), "die Bytes sind ein PDF");
    assert!(pdf.warnings.is_empty(), "nur eingebettete Schrift: keine Warnung, sonst {:?}", pdf.warnings);

    let png = compile_file(&job(input.clone(), OutputFormat::Png)).expect("PNG");
    assert_eq!(png.pages.len(), 2, "je Seite ein Bild (der Text hat einen Seitenumbruch)");
    for page in &png.pages {
        assert!(page.starts_with(PNG_SIGNATURE), "jede Seite trägt die PNG-Signatur");
    }
    // 105 mm = 297,64 pt; bei 144 ppi sind das 2 Pixel je Punkt.
    assert_eq!(png_width(&png.pages[0]), 595, "144 ppi: zwei Pixel je Punkt, wie bei Typst gerundet");
    let coarse = compile_file(&CompileJob { ppi: 72.0, ..job(input.clone(), OutputFormat::Png) }).expect("PNG 72");
    assert_eq!(png_width(&coarse.pages[0]), 298, "72 ppi: ein Pixel je Punkt – --ppi wirkt auf die Größe");

    let svg = compile_file(&job(input.clone(), OutputFormat::Svg)).expect("SVG");
    assert_eq!(svg.pages.len(), 2, "je Seite ein SVG");
    for page in &svg.pages {
        let text = String::from_utf8(page.clone()).expect("SVG ist Text");
        assert!(text.starts_with("<svg") && text.trim_end().ends_with("</svg>"), "jede Seite ist ein ganzes SVG");
    }
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, ["faehrmann.typ"], "die Engine schreibt nichts; das Schreiben ist Sache der Kommandozeile");
}

#[test]
fn the_root_allows_files_inside_and_blocks_everything_outside() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("projekt");
    write(&root, "teile/hafen.typ", "Im Hafen lag nur ein einziges Boot.\n");
    let input = write(&root, "kapitel/main.typ", "= Steinsee\n#include \"/teile/hafen.typ\"\n");
    write(dir.path(), "geheim.typ", "Das steht außerhalb des Projekts.\n");

    let inside = CompileJob { root: Some(root.clone()), ..job(input.clone(), OutputFormat::Svg) };
    let set = compile_file(&inside).expect("eine Datei im Wurzelordner darf eingebunden werden");
    assert!(
        String::from_utf8_lossy(&set.pages[0]).contains("<svg"),
        "mit --root auf dem Projektordner findet »/teile/hafen.typ« die Datei über dem Ordner der Eingabe"
    );

    // Ohne --root ist der Ordner der Eingabe die Wurzel: derselbe Text erreicht »teile« nicht mehr.
    let narrow = compile_file(&job(input.clone(), OutputFormat::Svg)).expect_err("teile/ liegt über der Wurzel");
    assert!(
        narrow.messages.iter().any(|m| m.contains("main.typ:2:") && m.contains("error:")),
        "ohne --root gilt der Ordner der Eingabe; die Meldung nennt die Zeile des #include: {:?}",
        narrow.messages
    );

    let escaping = write(&root, "kapitel/flucht.typ", "#include \"../../geheim.typ\"\n");
    let error = compile_file(&CompileJob { root: Some(root.clone()), ..job(escaping, OutputFormat::Pdf) })
        .expect_err("ein Pfad über die Wurzel hinaus ist gesperrt");
    assert!(
        error.messages.iter().any(|m| m.contains("flucht.typ:1:") && m.contains("error:")),
        "»../../geheim.typ« verlässt den Wurzelordner: Fehler statt Inhalt: {:?}",
        error.messages
    );
    assert!(
        !error.messages.iter().any(|m| m.contains("außerhalb des Projekts")),
        "der Text der gesperrten Datei erscheint in keiner Meldung"
    );

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("geheim.typ"), root.join("teile/tuer.typ")).unwrap();
        let through = write(&root, "kapitel/tuer.typ", "#include \"/teile/tuer.typ\"\n");
        let error = compile_file(&CompileJob { root: Some(root.clone()), ..job(through, OutputFormat::Pdf) })
            .expect_err("ein Symlink im Wurzelordner, der hinausführt, ist gesperrt");
        assert!(
            error.messages.iter().any(|m| m.contains("tuer.typ:1:") && m.contains("error:")),
            "der Pfad liegt lexikalisch im Projekt, sein Ziel nicht – geprüft wird das aufgelöste Ziel: {:?}",
            error.messages
        );
    }

    let outside = write(dir.path(), "fremd.typ", "Fremd.\n");
    let error = compile_file(&CompileJob { root: Some(root), ..job(outside, OutputFormat::Pdf) })
        .expect_err("die Eingabe selbst muss im Wurzelordner liegen");
    assert!(
        error.messages[0].contains("liegt nicht im Wurzelordner"),
        "deutsche Meldung statt eines Satzes mit fremder Wurzel: {:?}",
        error.messages
    );
}

#[test]
fn errors_name_file_line_and_column() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "kaputt.typ", "= Steinsee\n\nDer Wind #unbekannt() drehte.\n");
    let error = compile_file(&job(input.clone(), OutputFormat::Pdf)).expect_err("unbekannte Funktion");
    let wanted = format!("{}:3:11: error: ", input.display());
    assert!(
        error.messages[0].starts_with(&wanted),
        "pfad:zeile:spalte: error: text – Zeile 3, Spalte 11 ist das »unbekannt« nach dem #: {:?}",
        error.messages
    );

    write(dir.path(), "teil.typ", "Gut.\n\n#let x = (\n");
    let main = write(dir.path(), "haupt.typ", "#include \"teil.typ\"\n");
    let error = compile_file(&job(main, OutputFormat::Pdf)).expect_err("Syntaxfehler in der eingebundenen Datei");
    let wanted = format!("{}:3:", dir.path().join("teil.typ").display());
    assert!(
        error.messages[0].starts_with(&wanted) && error.messages[0].contains(": error: "),
        "der Ort ist die eingebundene Datei unter dem Wurzelordner, nicht die Hauptdatei: {:?}",
        error.messages
    );

    let missing = compile_file(&job(dir.path().join("fehlt.typ"), OutputFormat::Pdf)).expect_err("keine Datei");
    assert!(
        missing.messages[0].contains("fehlt.typ: error: Eingabedatei lässt sich nicht lesen"),
        "eine fehlende Eingabe ist ein Dateifehler mit deutschem Text: {:?}",
        missing.messages
    );
}

#[test]
fn packages_are_never_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "paket.typ", "#import \"@preview/steinsee:0.1.0\": boot\nText.\n");
    let error = compile_file(&job(input, OutputFormat::Pdf)).expect_err("Pakete gibt es nicht");
    assert!(
        error.messages[0].contains("paket.typ:1:") && error.messages[0].contains(baruch_render::OFFLINE),
        "die Meldung des Servers für Pakete (OFFLINE), mit Ort des Imports – ohne jeden Netzzugriff: {:?}",
        error.messages
    );
}

#[test]
fn font_names_list_the_embedded_families_sorted_and_once() {
    let names = font_names(&[]);
    for family in ["DejaVu Sans Mono", "Libertinus Serif", "New Computer Modern", "New Computer Modern Math"] {
        assert!(names.iter().any(|name| name == family), "{family} ist eingebettet und fehlt in {names:?}");
    }
    let mut sorted = names.clone();
    sorted.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    sorted.dedup();
    assert_eq!(names, sorted, "sortiert (ohne Unterschied von Groß und Klein) und ohne Doppelte");
    assert_eq!(
        font_names(&[PathBuf::from("/gibt/es/nicht/schriften")]),
        names,
        "ein Schriftordner, den es nicht gibt, ändert nichts und ist kein Fehler"
    );
}

/// Eine Schrift mit erfundenem Familiennamen: DejaVu Sans des Systems, in der Namenstabelle gleich lang umbenannt
/// (Prüfsummen der Tabellen liest der Schriftleser nicht). `None`, wenn das System die Datei nicht hat.
fn renamed_font() -> Option<Vec<u8>> {
    fn find(dir: &Path, depth: usize) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() && depth > 0 {
                if let Some(found) = find(&path, depth - 1) {
                    return Some(found);
                }
            } else if path.file_name().is_some_and(|name| name == "DejaVuSans.ttf") {
                return Some(path);
            }
        }
        None
    }
    let mut bytes = std::fs::read(find(Path::new("/usr/share/fonts"), 4)?).ok()?;
    let replace = |bytes: &mut Vec<u8>, from: &[u8], to: &[u8]| {
        let mut at = 0;
        while at + from.len() <= bytes.len() {
            if &bytes[at..at + from.len()] == from {
                bytes[at..at + to.len()].copy_from_slice(to);
                at += from.len();
            } else {
                at += 1;
            }
        }
    };
    let wide = |text: &str| text.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    replace(&mut bytes, &wide("DejaVu Sans"), &wide("Steinsee Tx"));
    replace(&mut bytes, b"DejaVu Sans", b"Steinsee Tx");
    Some(bytes)
}

#[test]
fn a_font_path_adds_its_fonts_for_this_run() {
    let Some(font) = renamed_font() else {
        eprintln!("ÜBERSPRUNGEN: /usr/share/fonts hat kein DejaVuSans.ttf, aus dem der Test eine Schrift ableitet");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let fonts = dir.path().join("schriften");
    std::fs::create_dir(&fonts).unwrap();
    std::fs::write(fonts.join("steinsee.ttf"), font).unwrap();
    let family = "Steinsee Tx";
    assert!(!font_names(&[]).iter().any(|name| name == family), "ohne den Ordner kennt niemand die erfundene Schrift");
    assert!(
        font_names(std::slice::from_ref(&fonts)).iter().any(|name| name == family),
        "mit dem Ordner steht sie in der Liste – der Ordner wird gelesen"
    );

    let input = write(dir.path(), "schrift.typ", "#set text(font: \"Steinsee Tx\")\nJonte ruderte.\n");
    let without = compile_file(&job(input.clone(), OutputFormat::Pdf)).expect("setzt mit Ersatzschrift");
    assert!(
        without.warnings.iter().any(|w| w.contains("schrift.typ:1:")
            && w.contains("warning:")
            && w.to_lowercase().contains("steinsee tx")),
        "ohne --font-path fehlt die Schrift: Warnung mit Ort, sonst {:?}",
        without.warnings
    );
    let with = compile_file(&CompileJob { font_paths: vec![fonts], ..job(input, OutputFormat::Pdf) }).expect("setzt");
    assert!(with.warnings.is_empty(), "mit --font-path ist die Schrift da: keine Warnung, sonst {:?}", with.warnings);
    assert!(
        !font_names(&[]).iter().any(|name| name == family),
        "der Ordner galt nur für diesen Lauf; der Vorrat des Prozesses (und damit des Servers) bleibt unverändert"
    );
}

#[test]
fn the_version_constant_matches_the_locked_typst() {
    let lock = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock")).unwrap();
    let at = lock.find("\nname = \"typst\"\n").expect("Cargo.lock führt das Crate typst");
    let rest = &lock[at + 1..];
    let version =
        rest.lines().nth(1).and_then(|line| line.strip_prefix("version = \"")).and_then(|v| v.strip_suffix('"'));
    assert_eq!(
        version,
        Some(TYPST_VERSION),
        "`typst --version` nennt TYPST_VERSION; hebt jemand das Crate an, muss die Konstante in \
         crates/baruch-render/src/cli.rs mit"
    );
}
