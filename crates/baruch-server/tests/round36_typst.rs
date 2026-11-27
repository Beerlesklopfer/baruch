// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“, Besitz typst_alias: Typst-Kommandozeile (`baruch-server typst`, Alias `baruch-typst`).
//!
//! - Die Kommandozeile selbst über `baruch_server::typst_cli::run` (ohne Prozess): `compile` zu PDF, PNG, SVG,
//!   `--root`, Satzfehler, `--version`, `fonts`, Nicht-Unterstütztes.
//! - Die gebaute Binärdatei über Symlinks `baruch-typst` und `typst` in einem Temp-Ordner und als
//!   `baruch-server typst`.
//! - Paketvorschriften (.deb, AppImage, .dmg, .msi), CI und Doku am Text; `AppRun` läuft gegen einen Nachbau.
//!
//! Alle Texte frei erfunden. Kein Server, kein Netz, kein Datenordner.

use std::path::{Path, PathBuf};
use std::process::Command;

use baruch_server::typst_cli::{ALIAS_NAMES, is_alias, run};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Eine Seite über die Glockengießerin von Harlund.
const BELL: &str = "#set page(width: 105mm, height: 148mm)\n= Die Glockengießerin von Harlund\n\nMarit goss die \
                    größte Glocke im Winter.\n";
/// Dasselbe mit einer zweiten Seite.
const BELL_TWO_PAGES: &str = "#set page(width: 105mm, height: 148mm)\n= Die Glockengießerin von Harlund\n\nMarit goss \
                              die größte Glocke im Winter.\n#pagebreak()\nIm Frühjahr läutete sie zum ersten Mal.\n";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_file(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Ruft die Kommandozeile im Prozess: (Exit-Code, Standardausgabe, Fehlerausgabe).
fn typst(args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn write(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_owned()
}

/// Die Dateinamen eines Ordners, sortiert (ohne Unterordner abzusteigen).
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn compile_sets_a_file_to_pdf_png_and_svg() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "glocke.typ", BELL);
    let path = |name: &str| dir.path().join(name).to_str().unwrap().to_owned();

    let (code, out, err) = typst(&["compile", &input]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "", ""),
        "Erfolg ist still wie bei Typst: Exit 0, keine Ausgabe"
    );
    let pdf = std::fs::read(path("glocke.pdf")).expect("ohne AUSGABE entsteht die Eingabe mit neuer Endung");
    assert!(pdf.starts_with(b"%PDF-"), "ohne --format und ohne AUSGABE entsteht ein PDF");

    assert_eq!(typst(&["c", &input, &path("bild.png")]).0, 0, "Kurzform c; das Format folgt der Endung .png");
    assert!(std::fs::read(path("bild.png")).unwrap().starts_with(PNG_SIGNATURE), "bild.png trägt die PNG-Signatur");

    assert_eq!(typst(&["compile", "--format", "svg", &input]).0, 0, "--format svg ohne AUSGABE");
    let svg = std::fs::read_to_string(path("glocke.svg")).expect("die Eingabe mit der Endung des Formats");
    assert!(svg.starts_with("<svg") && svg.contains("</svg>"), "glocke.svg ist ein SVG");

    assert_eq!(typst(&["compile", &input, &path("anders.dat"), "-f", "png"]).0, 0, "-f schlägt die Endung der Ausgabe");
    assert!(std::fs::read(path("anders.dat")).unwrap().starts_with(PNG_SIGNATURE), "anders.dat ist trotzdem ein PNG");

    // Überschreiben wie Typst (anders als `baruch-server export --pdf`, das eine vorhandene Datei ablehnt).
    std::fs::write(path("glocke.pdf"), "alt").unwrap();
    assert_eq!(typst(&["compile", &input]).0, 0, "eine vorhandene Ausgabedatei ist kein Fehler");
    assert!(
        std::fs::read(path("glocke.pdf")).unwrap().starts_with(b"%PDF-"),
        "compile überschreibt die vorhandene Ausgabe wie Typst (docs/Verteilung.md nennt den Unterschied zum Export)"
    );
    assert_eq!(
        names(dir.path()),
        ["anders.dat", "bild.png", "glocke.pdf", "glocke.svg", "glocke.typ"],
        "es entstehen genau die verlangten Dateien, nichts daneben"
    );
}

#[test]
fn several_pages_as_images_need_a_page_template() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "glocke.typ", BELL_TWO_PAGES);
    let path = |name: &str| dir.path().join(name).to_str().unwrap().to_owned();

    for format in ["png", "svg"] {
        let (code, _, err) = typst(&["compile", &input, "--format", format]);
        assert_eq!(code, 1, "{format}: zwei Seiten passen nicht in eine Bilddatei – Exit 1 (Vertrag §5)");
        assert!(
            err.contains("{p}") && err.contains("{0p}") && err.contains("{t}") && err.contains("nichts geschrieben"),
            "{format}: die Meldung nennt die Platzhalter: {err}"
        );
    }
    assert_eq!(names(dir.path()), ["glocke.typ"], "ohne Vorlage wird keine Seite geschrieben, auch nicht die erste");

    assert_eq!(
        typst(&["compile", &input, &path("seite-{p}-von-{t}.png")]).0,
        0,
        "mit {{p}} und {{t}} je Seite eine Datei"
    );
    assert_eq!(typst(&["compile", &input, &path("blatt-{0p}.svg")]).0, 0, "{{0p}} ebenso, für SVG");
    assert_eq!(
        names(dir.path()),
        ["blatt-1.svg", "blatt-2.svg", "glocke.typ", "seite-1-von-2.png", "seite-2-von-2.png"],
        "Seitennummern ab 1; {{0p}} füllt auf die Stellen der Seitenzahl auf (bei 2 Seiten eine Stelle)"
    );
    assert!(std::fs::read(path("seite-2-von-2.png")).unwrap().starts_with(PNG_SIGNATURE), "jede Seite ist ein PNG");
    assert_eq!(typst(&["compile", &input, &path("ganz.pdf")]).0, 0, "ein PDF fasst alle Seiten ohne Vorlage");
}

#[test]
fn the_root_allows_an_included_file_inside_and_blocks_one_outside() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("projekt");
    write(&project, "teile/guss.typ", "Die Form stand drei Tage in der Grube.\n");
    let input = write(&project, "kapitel/glocke.typ", "= Harlund\n#include \"/teile/guss.typ\"\n");
    write(dir.path(), "tagebuch.typ", "Niemand soll das lesen.\n");
    let out = dir.path().join("aus.pdf").to_str().unwrap().to_owned();

    let (code, _, err) = typst(&["compile", "--root", project.to_str().unwrap(), &input, &out]);
    assert_eq!(code, 0, "--root auf dem Projektordner: »/teile/guss.typ« liegt darin und wird eingebunden: {err}");
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF-"), "das PDF ist entstanden");
    std::fs::remove_file(&out).unwrap();

    let escaping = write(&project, "kapitel/flucht.typ", "#include \"../../tagebuch.typ\"\n");
    let (code, _, err) = typst(&["compile", &format!("--root={}", project.display()), &escaping, &out]);
    assert_eq!(code, 1, "eine Datei außerhalb von --root ist gesperrt: Exit 1 (Dateifehler)");
    assert!(err.contains("flucht.typ:1:") && err.contains("error:"), "mit Ort des #include: {err}");
    assert!(!err.contains("Niemand soll das lesen"), "der Inhalt der gesperrten Datei steht in keiner Meldung: {err}");
    assert!(!Path::new(&out).exists(), "und es wird nichts geschrieben");

    let (code, _, err) = typst(&["compile", &input, &out]);
    assert_eq!(code, 1, "ohne --root ist der Ordner der Eingabe die Wurzel; teile/ liegt darüber: {err}");
    assert!(!Path::new(&out).exists(), "auch dann entsteht keine Datei");

    let missing = dir.path().join("fehlt");
    let (code, _, err) = typst(&["compile", "--root", missing.to_str().unwrap(), &input, &out]);
    assert_eq!(code, 1, "ein Wurzelordner, den es nicht gibt, ist ein Dateifehler");
    assert!(err.contains("Wurzelordner (--root) gibt es nicht"), "deutsche Meldung: {err}");
}

#[test]
fn a_typesetting_error_names_file_line_and_column_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "riss.typ", "= Harlund\n\nDie Glocke #zersprang() beim ersten Schlag.\n");
    let out = dir.path().join("riss.pdf");
    std::fs::write(&out, "Stand von gestern").unwrap();

    let (code, stdout, err) = typst(&["compile", &input]);
    assert_eq!(code, 1, "Satzfehler: Exit 1");
    assert!(stdout.is_empty(), "Meldungen gehen auf die Fehlerausgabe, nicht auf die Standardausgabe");
    assert!(
        err.starts_with(&format!("{input}:3:13: error: ")),
        "datei:zeile:spalte: error: text – Zeile 3, Spalte 13 ist »zersprang«: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "Stand von gestern",
        "bei einem Satzfehler wird nichts geschrieben – auch eine vorhandene Ausgabe bleibt, wie sie war"
    );
    assert_eq!(names(dir.path()), ["riss.pdf", "riss.typ"], "keine halbe oder leere Datei daneben");

    let (code, _, err) = typst(&["compile", dir.path().join("gibt-es-nicht.typ").to_str().unwrap()]);
    assert_eq!(code, 1, "eine fehlende Eingabe ist ein Dateifehler (Exit 1), kein falscher Aufruf");
    assert!(err.contains("gibt-es-nicht.typ: error: Eingabedatei lässt sich nicht lesen"), "deutsche Meldung: {err}");
}

#[test]
fn the_version_names_the_embedded_typst_and_baruch() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = typst(&[flag]);
        assert_eq!(code, 0, "{flag}: eine Auskunft, kein Fehler");
        assert_eq!(
            out,
            format!("typst {} (Baruch {})\n", baruch_render::cli::TYPST_VERSION, env!("CARGO_PKG_VERSION")),
            "{flag}: »typst <Version>« vorn wie bei Typst (Skripte lesen das zweite Wort), dahinter ehrlich, wer antwortet"
        );
        assert!(err.is_empty(), "{flag}: nichts auf der Fehlerausgabe");
    }
    assert!(typst(&["--version"]).1.starts_with("typst 0.15.1 (Baruch "), "Stand der Runde 36: Engine 0.15.1");
    let lock = repo_file("Cargo.lock");
    let rest = &lock[lock.find("\nname = \"typst\"\n").expect("Cargo.lock führt das Crate typst") + 1..];
    assert_eq!(
        rest.lines().nth(1),
        Some(format!("version = \"{}\"", baruch_render::cli::TYPST_VERSION).as_str()),
        "TYPST_VERSION ist die Version des Crates typst in Cargo.lock – sonst nennte --version einen anderen Compiler"
    );
}

#[test]
fn fonts_lists_at_least_the_embedded_families() {
    let (code, out, err) = typst(&["fonts"]);
    assert_eq!((code, err.as_str()), (0, ""), "fonts: Exit 0 ohne Meldung");
    let lines: Vec<&str> = out.lines().collect();
    for family in ["DejaVu Sans Mono", "Libertinus Serif", "New Computer Modern", "New Computer Modern Math"] {
        assert!(lines.contains(&family), "{family} ist mit Baruch eingebettet und steht als eigene Zeile da: {out}");
    }
    let dir = tempfile::tempdir().unwrap();
    let (code, with_path, _) = typst(&["fonts", "--font-path", dir.path().to_str().unwrap()]);
    assert_eq!(code, 0, "fonts --font-path mit einem leeren Ordner");
    assert_eq!(
        with_path, out,
        "ein leerer Schriftordner ändert die Liste nicht (gelesen wird er in baruch-render::cli)"
    );
    let (code, _, err) = typst(&["fonts", "--variants"]);
    assert_eq!(code, 2, "--variants kennt die eingebaute Kommandozeile nicht");
    assert!(err.contains("„--variants“ unterstützt Baruch als Typst-Alias nicht"), "ehrlich benannt: {err}");
}

#[test]
fn unsupported_calls_are_refused_in_german_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(dir.path(), "glocke.typ", BELL);
    let out = dir.path().join("glocke.pdf").to_str().unwrap().to_owned();
    let refused = "unterstützt Baruch als Typst-Alias nicht";

    for command in ["watch", "query", "init", "update", "eval", "completions", "info"] {
        let (code, stdout, err) = typst(&[command, &input, &out]);
        assert_eq!(code, 2, "{command}: gibt es nicht – Exit 2 (Nicht-Unterstütztes)");
        assert!(err.contains(&format!("Den Unterbefehl „{command}“ {refused}")), "{command}: deutsche Meldung: {err}");
        assert!(stdout.is_empty(), "{command}: nichts auf der Standardausgabe");
    }
    for flags in [&["--open"][..], &["--pdf-standard", "a-2b"], &["--input", "x=1"], &["--pages", "1"], &["--jobs=2"]] {
        let mut args = vec!["compile", input.as_str(), out.as_str()];
        args.extend_from_slice(flags);
        let (code, _, err) = typst(&args);
        assert_eq!(code, 2, "{flags:?}: ein Schalter, den Baruch nicht umsetzt, wird nicht still übergangen");
        assert!(err.contains(&format!("„{}“ {refused}", flags[0].split('=').next().unwrap())), "{flags:?}: {err}");
    }
    let (code, _, err) = typst(&["compile", "-", &out]);
    assert_eq!(code, 2, "Eingabe von der Standardeingabe: Exit 2");
    assert!(err.contains(&format!("Die Eingabe von der Standardeingabe (-) {refused}")), "deutsche Meldung: {err}");
    let (code, _, err) = typst(&["compile", &input, "-"]);
    assert_eq!(code, 2, "Ausgabe auf die Standardausgabe: Exit 2");
    assert!(err.contains(refused), "deutsche Meldung: {err}");
    let (code, _, err) = typst(&["compile", &input, dir.path().join("seite.html").to_str().unwrap()]);
    assert_eq!(code, 2, "HTML-Ausgabe: Exit 2 statt einer falsch benannten PDF-Datei");
    assert!(err.contains(refused), "deutsche Meldung: {err}");
    let (code, _, err) = typst(&["--color", "always", "compile", &input]);
    assert_eq!(code, 2, "ein unbekannter Schalter vor dem Unterbefehl");
    assert!(err.contains(&format!("Den Schalter „--color“ {refused}")), "deutsche Meldung: {err}");
    let (code, _, err) = typst(&["setze", &input]);
    assert_eq!(code, 2, "ein unbekanntes Wort ist ein falscher Aufruf");
    assert!(err.contains("unbekannter Unterbefehl „setze“") && err.contains("typst --help"), "mit Hinweis: {err}");
    assert_eq!(typst(&[]).0, 2, "ohne jede Angabe: Exit 2 mit der Hilfe");

    // Pakete: kein falscher Aufruf, sondern ein Dateifehler beim Satz – Exit 1 mit der Meldung des Servers.
    let package = write(dir.path(), "paket.typ", "#import \"@preview/harlund:0.1.0\": glocke\nText.\n");
    let (code, _, err) = typst(&["compile", &package]);
    assert_eq!(code, 1, "ein @preview-Paket: Exit 1");
    assert!(
        err.contains("paket.typ:1:") && err.contains(baruch_render::OFFLINE),
        "die feste Meldung OFFLINE – die Welt der Kommandozeile hat keinen Weg ins Netz, sie antwortet sofort: {err}"
    );

    assert_eq!(names(dir.path()), ["glocke.typ", "paket.typ"], "keiner dieser Aufrufe hat eine Datei geschrieben");

    let (code, help, err) = typst(&["--help"]);
    assert_eq!((code, err.as_str()), (0, ""), "--help: Exit 0 auf der Standardausgabe");
    for word in ["compile", "--root", "--font-path", "--format", "fonts", "überschrieben", "watch", "@preview"] {
        assert!(help.contains(word), "die Hilfe nennt {word}: {help}");
    }
}

/// Startet die gebaute Binärdatei unter `program` mit eigenem HOME (damit kein Datenordner des Rechners in Reichweite
/// ist) und ohne `TYPST_FONT_PATHS`.
fn process(program: &Path, home: &Path, args: &[&str]) -> std::process::Output {
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

#[cfg(unix)]
#[test]
fn the_built_program_answers_as_typst_under_its_alias_names() {
    let server = Path::new(env!("CARGO_BIN_EXE_baruch-server"));
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    for name in ["baruch-typst", "typst", "setzer"] {
        std::os::unix::fs::symlink(server, bin.join(name)).unwrap();
    }
    let input = write(dir.path(), "werk/glocke.typ", BELL);
    let version = format!("typst {} (Baruch {})\n", baruch_render::cli::TYPST_VERSION, env!("CARGO_PKG_VERSION"));

    // Dreimal dasselbe: Symlink baruch-typst (so liefern es die Pakete), Symlink typst, Unterbefehl typst.
    let ways: [(&str, PathBuf, &[&str]); 3] = [
        ("baruch-typst", bin.join("baruch-typst"), &[]),
        ("typst", bin.join("typst"), &[]),
        ("baruch-server typst", server.to_path_buf(), &["typst"]),
    ];
    let mut pages = Vec::new();
    for (name, program, prefix) in &ways {
        let call = |args: &[&str]| process(program, &home, &[prefix, args].concat());
        let out = call(&["--version"]);
        assert_eq!(out.status.code(), Some(0), "{name} --version: Exit 0");
        assert_eq!(String::from_utf8_lossy(&out.stdout), version, "{name}: die Zeile der Typst-Kommandozeile");

        let svg = dir.path().join(format!("{}.svg", name.replace(' ', "-")));
        let out = call(&["compile", &input, svg.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "{name} compile: {}", String::from_utf8_lossy(&out.stderr));
        pages.push(std::fs::read(&svg).expect("die Ausgabe liegt da"));

        let pdf = dir.path().join(format!("{}.pdf", name.replace(' ', "-")));
        let out = call(&["compile", &input, pdf.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "{name} compile zu PDF");
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"), "{name}: ein PDF");

        let out = call(&["watch", &input]);
        assert_eq!(out.status.code(), Some(2), "{name} watch: Exit 2 auch als Prozess");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("unterstützt Baruch als Typst-Alias nicht"),
            "{name} watch: die deutsche Meldung auf der Fehlerausgabe"
        );
    }
    assert!(pages[0].starts_with(b"<svg"), "die Ausgabe ist ein SVG");
    assert!(
        pages[0] == pages[1] && pages[1] == pages[2],
        "alle drei Wege setzen mit derselben Engine dasselbe SVG (Byte für Byte)"
    );

    // TYPST_FONT_PATHS wird gelesen wie --font-path (ein leerer Ordner ändert nichts und ist kein Fehler).
    let out = Command::new(bin.join("baruch-typst"))
        .arg("fonts")
        .env("HOME", &home)
        .env("TYPST_FONT_PATHS", dir.path().join("werk"))
        .output()
        .expect("baruch-typst startet");
    assert_eq!(out.status.code(), Some(0), "fonts mit TYPST_FONT_PATHS: Exit 0");
    assert!(String::from_utf8_lossy(&out.stdout).lines().any(|line| line == "Libertinus Serif"), "die Liste steht da");

    // Ein anderer Name ist kein Alias: Dort antwortet der Server wie immer.
    let out = process(&bin.join("setzer"), &home, &["--version"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("baruch-server {}\n", env!("CARGO_PKG_VERSION")),
        "unter einem fremden Namen startet nicht die Typst-Kommandozeile"
    );
    let out = process(server, &home, &["--version"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("baruch-server {}\n", env!("CARGO_PKG_VERSION")),
        "baruch-server --version ist unverändert"
    );
    let out = process(server, &home, &["serve", "--allow-host"]);
    assert_eq!(out.status.code(), Some(2), "serve prüft seine Angaben wie bisher (hier: --allow-host ohne Namen)");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--allow-host braucht einen Rechnernamen"),
        "serve landet beim Server, nicht bei der Typst-Kommandozeile"
    );
    let out = process(server, &home, &["--help"]);
    let help = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        help.contains("baruch-server typst compile EINGABE.typ") && help.contains("baruch-typst"),
        "die Hilfe des Servers nennt den Unterbefehl und den Aliasnamen: {help}"
    );
    assert!(
        help.contains("eine Sperre\n            nach Fehlversuchen wird aufgehoben"),
        "die Hilfe zu passwort nennt, dass die Sperre fällt (Vertrag §3): {help}"
    );
    assert_eq!(
        names(&home),
        Vec::<String>::new(),
        "keiner der Aufrufe hat im Benutzerordner etwas angelegt – kein Datenordner, kein Server"
    );
}

#[test]
fn alias_names_cover_exe_and_paths_but_no_other_name() {
    assert_eq!(ALIAS_NAMES, ["typst", "baruch-typst"], "die beiden Namen des Vertrags");
    for argv0 in ["baruch-typst", "/usr/bin/baruch-typst", "bin/typst", "BARUCH-TYPST.EXE", "Typst.exe"] {
        assert!(is_alias(std::ffi::OsStr::new(argv0)), "{argv0}: Pfad und .exe (Groß/Klein egal) zählen nicht");
    }
    for argv0 in ["baruch-server", "baruch", "/opt/baruch-typst/baruch-server", "baruch-typst.cmd", "typst2"] {
        assert!(!is_alias(std::ffi::OsStr::new(argv0)), "{argv0}: kein Aliasname");
    }
}

// ---- Pakete ----------------------------------------------------------------------------------------------------

/// Die Zeilen `["quelle", "ziel", "modus"]` aus `[package.metadata.deb] assets`.
fn deb_assets() -> Vec<(String, String, String)> {
    let manifest = repo_file("crates/baruch-server/Cargo.toml");
    let deb = &manifest[manifest.find("[package.metadata.deb]").expect("Abschnitt deb")..];
    let deb = &deb[..deb.find("\n[dependencies]").expect("Ende des Abschnitts")];
    let assets = &deb[deb.find("assets = [").expect("assets")..];
    assets
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.trim().strip_prefix("[\"")?.strip_suffix("\"],")?.split("\", \"").collect();
            (parts.len() == 3).then(|| (parts[0].to_owned(), parts[1].to_owned(), parts[2].to_owned()))
        })
        .collect()
}

#[test]
fn the_deb_ships_baruch_typst_as_a_symlink_to_the_server() {
    let manifest = repo_file("crates/baruch-server/Cargo.toml");
    let assets = deb_assets();
    assert!(assets.len() >= 13, "die Assets sind lesbar (sonst prüfte dieser Test nichts): {assets:?}");
    let link: Vec<_> = assets.iter().filter(|(_, dest, _)| dest == "usr/bin/baruch-typst").collect();
    assert_eq!(link.len(), 1, "genau ein Eintrag für /usr/bin/baruch-typst");
    assert_eq!(link[0].0, "../../packaging/linux/baruch-typst", "seine Quelle ist der Symlink im Repo");
    assert!(
        manifest.contains("\npreserve-symlinks = true\n"),
        "ohne preserve-symlinks kopierte cargo-deb das Ziel – das Paket trüge den Server zweimal"
    );
    #[cfg(unix)]
    {
        let source = root().join("packaging/linux/baruch-typst");
        assert_eq!(
            std::fs::read_link(&source).expect("packaging/linux/baruch-typst ist ein Symlink"),
            Path::new("baruch-server"),
            "relativ auf den Nachbarn: in /usr/bin zeigt er auf /usr/bin/baruch-server, wo auch immer das Paket liegt"
        );
    }
    assert!(
        assets
            .iter()
            .any(|(source, dest, _)| source == "target/release/baruch-server" && dest == "usr/bin/baruch-server"),
        "das Ziel des Symlinks liegt im selben Ordner des Pakets"
    );
}

#[test]
fn no_package_installs_a_file_named_typst() {
    for (source, dest, _) in deb_assets() {
        let name = dest.rsplit('/').next().unwrap();
        assert!(
            !["typst", "typst.exe"].contains(&name),
            ".deb: {source} → {dest} – ein fremdes /usr/bin/typst darf das Paket nie überschreiben"
        );
    }
    // Skripte und Vorschriften: Kein Ziel heißt typst. Geprüft werden die Zeilen, die etwas ablegen.
    for path in [
        "packaging/appimage/build-appimage.sh",
        "packaging/macos/build-dmg.sh",
        "packaging/windows/baruch.wxs",
        "packaging/windows/build-msi.ps1",
        "scripts/build-deb.sh",
    ] {
        let text = repo_file(path);
        for line in text.lines().filter(|line| !line.trim_start().starts_with('#')) {
            for forbidden in ["/typst\"", "/typst ", "\\typst.exe", "Name=\"typst", "typst.cmd\"", "/typst.exe"] {
                let hit = line.contains(forbidden) && !line.contains("baruch-typst");
                assert!(!hit, "{path}: »{}« legt eine Datei namens typst ab", line.trim());
            }
        }
    }
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if ["typst", "typst.exe", "typst.cmd", "typst.bat"].contains(&name.as_str()) {
                found.push(path.clone());
            }
            if path.is_dir() && !path.is_symlink() {
                walk(&path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(&root().join("packaging"), &mut found);
    assert_eq!(found, Vec::<PathBuf>::new(), "unter packaging/ liegt nichts, das typst heißt");
}

#[test]
fn appimage_dmg_and_msi_carry_baruch_typst() {
    let appimage = repo_file("packaging/appimage/build-appimage.sh");
    assert!(
        appimage.contains("\nln -s baruch-server \"$appdir/usr/bin/baruch-typst\"\n"),
        "AppImage: usr/bin/baruch-typst als Symlink auf den Nachbarn baruch-server"
    );
    let dmg = repo_file("packaging/macos/build-dmg.sh");
    assert!(
        dmg.contains("\nln -s baruch-server \"$app/Contents/MacOS/baruch-typst\"\n"),
        ".dmg: Contents/MacOS/baruch-typst als Symlink auf den Nachbarn baruch-server"
    );
    assert!(
        dmg.find("Contents/MacOS/baruch-typst\"").unwrap() < dmg.find("codesign --force").unwrap(),
        ".dmg: der Symlink entsteht, bevor signiert wird (sonst bräche er die Signatur des Bündels)"
    );
    let wxs = repo_file("packaging/windows/baruch.wxs");
    assert!(
        wxs.contains("Name=\"baruch-typst.cmd\"")
            && wxs.contains("Source=\"$(var.SourceDir)\\packaging\\windows\\baruch-typst.cmd\""),
        ".msi: baruch-typst.cmd liegt neben baruch-server.exe (dieselbe ComponentGroup)"
    );
    assert_eq!(wxs.matches("<Component Id=\"TypstCmd\"").count(), 1, ".msi: genau eine Komponente dafür");
    let cmd = repo_file("packaging/windows/baruch-typst.cmd");
    assert!(cmd.is_ascii(), "cmd.exe liest die Datei in der Codepage des Systems: nur ASCII");
    assert!(
        cmd.contains("\n\"%~dp0baruch-server.exe\" typst %*\n"),
        "reicht alle Argumente an baruch-server.exe im eigenen Ordner weiter (Vertrag §5)"
    );
    assert!(cmd.contains("\nexit /b %ERRORLEVEL%\n"), "und gibt dessen Exit-Code zurück (1 Satzfehler, 2 Aufruf)");
}

/// `AppRun` gegen einen Nachbau von `baruch-server`, der seine Argumente nennt: Die Laufzeit des AppImage setzt
/// `ARGV0` auf den Namen, unter dem das AppImage gerufen wurde.
#[cfg(unix)]
#[test]
fn apprun_passes_the_alias_names_on_to_the_typst_command_line() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("usr/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let executable = |path: &Path, text: &str| {
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    executable(&bin.join("baruch-server"), "#!/bin/sh\necho \"server:$*\"\n");
    executable(&bin.join("baruch"), "#!/bin/sh\necho \"starter:$*\"\n");
    executable(&dir.path().join("AppRun"), &repo_file("packaging/appimage/AppRun"));
    let apprun = |argv0: Option<&str>, args: &[&str]| {
        let mut command = Command::new(dir.path().join("AppRun"));
        command.args(args).env_remove("ARGV0");
        if let Some(argv0) = argv0 {
            command.env("ARGV0", argv0);
        }
        String::from_utf8(command.output().expect("AppRun startet").stdout).unwrap()
    };
    assert_eq!(
        apprun(Some("/home/marit/bin/baruch-typst"), &["compile", "glocke.typ"]),
        "server:typst compile glocke.typ\n",
        "ARGV0 baruch-typst (Symlink auf das AppImage): baruch-server typst mit allen Argumenten"
    );
    assert_eq!(apprun(Some("typst"), &["--version"]), "server:typst --version\n", "ARGV0 typst ebenso");
    assert_eq!(
        apprun(Some("baruch-typst"), &[]),
        "server:typst\n",
        "auch ohne Argumente die Typst-Kommandozeile (ihre Hilfe), nicht der Starter mit Browser"
    );
    assert_eq!(
        apprun(Some("./Baruch-0.0.0-x86_64.AppImage"), &["--version"]),
        "server:--version\n",
        "unter dem eigenen Namen bleibt alles wie bisher: Argumente unverändert an baruch-server"
    );
    assert_eq!(apprun(Some("./Baruch-0.0.0-x86_64.AppImage"), &[]), "starter:\n", "ohne Argumente der Starter");
    assert_eq!(apprun(None, &["export", "1"]), "server:export 1\n", "ohne ARGV0 (entpacktes AppDir) wie bisher");
}

// ---- CI und Doku -----------------------------------------------------------------------------------------------

#[test]
fn the_ci_sets_with_its_own_program_and_downloads_no_typst() {
    let ci = repo_file(".github/workflows/ci.yml");
    assert!(
        ci.contains("\n          sh scripts/ci-typst.sh target/debug/baruch-server\n"),
        "ci.yml: ci-typst.sh ohne zweites Argument – dann setzt das eigene Programm über den Symlink baruch-typst"
    );
    for workflow in ["ci.yml", "release.yml", "pages.yml"] {
        let text = repo_file(&format!(".github/workflows/{workflow}"));
        assert!(
            !text.contains("ci-tool.sh typst") && !text.contains("typst/typst/releases"),
            "{workflow}: lädt kein Typst mehr"
        );
    }
    let tool = repo_file("scripts/ci-tool.sh");
    assert!(
        !tool.contains("TYPST_SHA256") && !tool.contains("typst-x86_64"),
        "ci-tool.sh: Version, Prüfsumme und Adresse von Typst sind entfernt; Hugo bleibt"
    );
    assert!(tool.contains("HUGO_SHA256=\""), "ci-tool.sh: Hugo wird weiter mit Prüfsumme geladen");
    let script = repo_file("scripts/ci-typst.sh");
    assert!(
        script.contains("ln -s \"$server\" \"$work/bin/baruch-typst\""),
        "ci-typst.sh: der Symlink baruch-typst im Temp-Ordner – derselbe Weg wie in den Paketen"
    );
    assert!(
        script.contains("\"typst $locked (Baruch \"*\")\""),
        "ci-typst.sh: --version wird gegen die Typst-Version aus Cargo.lock gehalten"
    );
}

#[test]
fn pages_are_delivered_again_when_a_release_is_published() {
    let pages = repo_file(".github/workflows/pages.yml");
    assert!(
        pages.contains(
            "\non:\n  push:\n    branches: [main]\n  release:\n    types: [published]\n  workflow_dispatch:\n"
        ),
        "pages.yml: neben Push auf main und dem Handweg läuft der Ablauf, wenn ein Release veröffentlicht wird – so \
         kommt die Seite auch an, wenn der Tag auf dem Commit des letzten main-Laufs liegt"
    );
    assert!(
        pages.contains("\nconcurrency:\n  group: pages\n  cancel-in-progress: false\n"),
        "pages.yml: die Gruppe pages bleibt am ganzen Lauf"
    );
    assert!(
        pages.contains("          ref: ${{ github.event_name == 'release' && 'main' || '' }}\n"),
        "pages.yml: beim Ereignis release wird main gebaut, nicht der Stand des Tags (ein spät veröffentlichtes \
         Release setzte die Seite sonst zurück)"
    );
    assert_eq!(pages.matches("ref: ").count(), 1, "pages.yml: nur der eine Checkout im bauenden Job trägt ref");
    let doc = repo_file("docs/Verteilung.md");
    for wanted in [
        "### Auslieferung je Commit",
        "release: types: [published]",
        "gh workflow run pages.yml",
        "RELEASE_DRAFT=false",
        "Nie ausgeführt geprüft",
    ] {
        assert!(
            doc.contains(wanted),
            "Verteilung.md nennt Vorgehen, Handweg, Grenze und den Stand der Prüfung: {wanted}"
        );
    }
}

#[test]
fn the_docs_describe_the_typst_command_line() {
    let doc = repo_file("docs/Verteilung.md");
    let at = doc.find("\n## Typst-Kommandozeile\n").expect("Verteilung.md: Abschnitt „Typst-Kommandozeile“");
    let section = &doc[at..at + doc[at + 1..].find("\n## ").expect("Folgeabschnitt") + 1];
    for wanted in [
        "baruch-typst",
        "baruch-server typst",
        "`--root ORDNER`",
        "`--font-path ORDNER`",
        "TYPST_FONT_PATHS",
        "überschreibt",
        "`watch`",
        "@preview",
        "Exit 2",
        "/usr/bin/baruch-typst",
        "baruch-typst.cmd",
        "Contents/MacOS/baruch-typst",
        "nie eine Datei namens `typst`",
    ] {
        assert!(section.contains(wanted), "Verteilung.md, „Typst-Kommandozeile“: {wanted} fehlt");
    }
    assert!(
        !doc.contains("scripts/ci-tool.sh typst")
            && !doc.contains("| Typst | 0.15.1 (= Crate `typst` in `Cargo.lock`) | `scripts/ci-tool.sh`"),
        "Verteilung.md: der Download von Typst steht nicht mehr unter den festgelegten Werkzeugen"
    );
    for (path, wanted) in [
        ("site/content/installation.md", "baruch-typst compile"),
        ("site/content/funktionen.md", "baruch-typst compile"),
        ("README.md", "baruch-typst compile"),
        ("packaging/debian/README.Debian", "baruch-typst compile"),
    ] {
        assert!(repo_file(path).contains(wanted), "{path} nennt {wanted}");
    }
}
