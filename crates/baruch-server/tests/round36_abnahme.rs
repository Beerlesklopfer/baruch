// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 36 „Reste“, owner integrator: acceptance across the boundaries of the three tasks.
//!
//! The built binary exports a work (Typst file including image, and as PDF); the built-in Typst command line – as the
//! symlink `baruch-typst` and as `baruch-server typst` – typesets the exported Typst file to the same PDF. The
//! remaining acceptance items of the round are with their owners (`round36_typst`, `round36_sqlx`, `round36_lektor`,
//! `round36_mindmap`, `round36_reste`, `round35_konten`).
//!
//! All texts freely invented. No server, no network; the data directory is a temp directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SIGN: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\" viewBox=\"0 0 40 20\">\
                    <rect width=\"40\" height=\"20\" fill=\"#2f5d50\"/></svg>";

const BOOK: &str = "#set document(title: \"Die Fähre nach Skarvik\", author: \"Inga Tolland\")\n\
                    #set page(width: 12cm, height: 16cm)\n#set text(lang: \"de\")\n\n= Anlegen\n\n\
                    Die Fähre legte bei Nebel an, und niemand stieg aus.\n\n#image(\"zeichen.svg\", width: 3cm)\n\n\
                    #pagebreak()\n\n= Ablegen\n\nGegen Abend nahm sie drei Kisten Äpfel und einen Brief mit.\n";

/// Starts the built binary under `program` with its own HOME, so that no data directory of the machine is within reach.
fn process(program: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env_remove("TYPST_FONT_PATHS")
        .env_remove("BARUCH_DATA")
        .env_remove("BARUCH_LISTEN")
        .env_remove("BARUCH_ALLOWED_HOSTS")
        .output()
        .unwrap_or_else(|e| panic!("{} startet: {e}", program.display()))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn typst_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "typ"))
        .collect();
    found.sort();
    found
}

#[cfg(unix)]
#[test]
fn the_typst_alias_sets_an_exported_work_to_the_same_pdf_as_the_export() {
    let server = Path::new(env!("CARGO_BIN_EXE_baruch-server"));
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let data = dir.path().join("daten");
    let data_arg = data.to_str().unwrap();
    let source = dir.path().join("quelle");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("faehre.typ"), BOOK).unwrap();
    std::fs::write(source.join("zeichen.svg"), SIGN).unwrap();
    let alias = dir.path().join("baruch-typst");
    std::os::unix::fs::symlink(server, &alias).unwrap();

    let out = process(server, &home, &["import", source.join("faehre.typ").to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "der Import legt das Werk an: {}", text(&out.stderr));
    let slug = "die-faehre-nach-skarvik";

    // The export of Baruch: once as PDF, once as a Typst file including the image.
    let exported = dir.path().join("export.pdf");
    let out = process(server, &home, &["export", slug, "--pdf", exported.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --pdf gelingt (der Titel ergibt den slug {slug}): {}", text(&out.stderr));
    let folder = dir.path().join("ordner");
    let out = process(server, &home, &["export", slug, "--out", folder.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --out gelingt: {}", text(&out.stderr));
    let files = typst_files(&folder);
    assert_eq!(files.len(), 1, "der Export schreibt genau eine Typst-Datei in den Ordner: {files:?}");
    assert!(folder.join("zeichen.svg").is_file(), "und das Bild daneben, sonst fände die Kommandozeile es nicht");
    let input = files[0].to_str().unwrap();
    let expected = std::fs::read(&exported).unwrap();
    assert!(expected.starts_with(b"%PDF-"), "der Export ist ein PDF");

    // The same engine, the same fonts, the same source: both ways of the command line deliver byte for byte the PDF of
    // the export (the work has no cover and no front matter, so the frame of the export is empty).
    let by_alias = dir.path().join("alias.pdf");
    let out = process(&alias, &home, &["compile", input, by_alias.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "baruch-typst compile x.typ x.pdf: {}", text(&out.stderr));
    let by_command = dir.path().join("befehl.pdf");
    let out = process(server, &home, &["typst", "compile", input, by_command.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "baruch-server typst compile: {}", text(&out.stderr));
    let (alias_pdf, command_pdf) = (std::fs::read(&by_alias).unwrap(), std::fs::read(&by_command).unwrap());
    assert!(alias_pdf == command_pdf, "Symlink und Unterbefehl sind derselbe Weg und schreiben dieselbe Datei");
    assert!(
        alias_pdf == expected,
        "die Kommandozeile setzt den Export zu derselben PDF wie `export --pdf` ({} gegen {} Bytes)",
        alias_pdf.len(),
        expected.len()
    );

    // Cross-check: `watch` does not exist – honest message, exit 2, and no file is created.
    let before = std::fs::read_dir(&folder).unwrap().count();
    let out = process(&alias, &home, &["watch", input]);
    assert_eq!(out.status.code(), Some(2), "watch ist Nicht-Unterstütztes: Exit 2");
    assert!(
        text(&out.stderr).contains("unterstützt Baruch als Typst-Alias nicht"),
        "die Meldung sagt ehrlich, dass es das nicht gibt: {}",
        text(&out.stderr)
    );
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), before, "watch schreibt nichts neben die Eingabe");
    assert!(!home.join("data").exists(), "die Typst-Kommandozeile legt keinen Datenordner an");
}
