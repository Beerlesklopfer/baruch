// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst command line of Baruch (Round 36, ownership `typst_alias`): `baruch-server typst …` or the program called
//! under one of the names from [`ALIAS_NAMES`] (symlink `baruch-typst`). The engine is in `baruch_render::cli`,
//! the arguments are here.
//!
//! Scope: `compile EINGABE [AUSGABE]` (short form `c`) with `--root`, `--font-path` (several times, also
//! `TYPST_FONT_PATHS`), `--format pdf|png|svg` (`-f`) and `--ppi`; `fonts`; `--version`; `--help`. Everything else
//! of Typst (watch, query, init, update, eval, completions, info, further switches, input from the standard input)
//! is honestly rejected before anything is read or written. Packages and network do not exist
//! ([`baruch_render::OFFLINE`]). Like Typst – and unlike the export of Baruch – `compile` overwrites an
//! existing output file; but writing happens only when the whole typesetting has succeeded.
//!
//! Exit codes (fixed): 0 success; 1 typesetting or file error; 2 wrong call or something unsupported.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};

use baruch_render::cli::{CompileJob, OutputFormat, TYPST_VERSION, compile_file, font_names};

/// Names under which the program runs as the Typst command line.
pub const ALIAS_NAMES: &[&str] = &["typst", "baruch-typst"];

/// File name of `argv[0]` without folder and without ".exe" (case does not matter) is one from [`ALIAS_NAMES`].
pub fn is_alias(argv0: &OsStr) -> bool {
    let Some(name) = Path::new(argv0).file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let name = name.to_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    ALIAS_NAMES.contains(&name)
}

/// `args` including `argv[0]`. `Some(exit code)` if called as an alias (then `args[1..]` apply) or
/// `args[1] == "typst"` (then `args[2..]`); otherwise `None`.
pub fn dispatch(args: Vec<OsString>) -> Option<i32> {
    let skip = if args.first().is_some_and(|argv0| is_alias(argv0)) {
        1
    } else if args.get(1).is_some_and(|first| first == "typst") {
        2
    } else {
        return None;
    };
    let rest: Vec<String> = args[skip..].iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
    Some(run(&rest, &mut std::io::stdout().lock(), &mut std::io::stderr().lock()))
}

/// Help of the Typst command line (`--help`).
pub const HELP: &str = "Typst-Kommandozeile von Baruch (eingebaute Engine, ohne Netz)

typst compile EINGABE.typ [AUSGABE] [--root ORDNER] [--font-path ORDNER]… [--format pdf|png|svg] [--ppi ZAHL]
typst fonts [--font-path ORDNER]…
typst --version
typst --help

Aufruf als baruch-typst … oder baruch-server typst …
  compile     (kurz: c) eine Typst-Datei setzen. Ohne AUSGABE entsteht die Datei neben der Eingabe mit der
              Endung des Formats. Eine vorhandene Ausgabedatei wird überschrieben (wie bei Typst); bei einem
              Satzfehler wird nichts geschrieben.
  --root      Wurzelordner des Projekts (Vorgabe: Ordner der Eingabe); Dateien außerhalb sind gesperrt
  --font-path Ordner mit weiteren Schriften (mehrfach; auch TYPST_FONT_PATHS), zusätzlich zu den
              eingebetteten und den Systemschriften
  --format    (kurz: -f) pdf, png oder svg; ohne Angabe nach der Endung der Ausgabe, sonst pdf
  --ppi       Auflösung für png (Vorgabe 144)
              png und svg mit mehreren Seiten brauchen {p}, {0p} oder {t} im Namen der Ausgabe
              (Seitennummer, Seitennummer mit führenden Nullen, Seitenzahl), z. B. seite-{0p}.png
  fonts       die Familiennamen der verfügbaren Schriften, eine je Zeile
  --version   typst VERSION (Baruch VERSION)
Nicht unterstützt: watch, query, init, update, eval, completions, info, Eingabe von der Standardeingabe (-),
Pakete (@preview/…) und alles aus dem Netz, weitere Schalter von Typst.
Exit: 0 Erfolg, 1 Satz- oder Dateifehler, 2 falscher Aufruf oder Nicht-Unterstütztes.";

/// Subcommands of Typst that do not exist here.
const UNSUPPORTED_COMMANDS: &[&str] = &["watch", "w", "query", "init", "update", "eval", "completions", "info"];

/// A wrong call or something unsupported: message for `err`, exit 2.
#[derive(Debug, PartialEq, Eq)]
struct Usage(String);

fn unsupported(what: &str) -> Usage {
    Usage(format!("{what} unterstützt Baruch als Typst-Alias nicht"))
}

/// The details read from a call of `compile` or `fonts`.
#[derive(Default)]
struct Options {
    positional: Vec<String>,
    root: Option<PathBuf>,
    font_paths: Vec<PathBuf>,
    format: Option<String>,
    ppi: Option<String>,
}

/// Reads switches and free arguments; `allowed` names the switches with a value that the subcommand knows.
/// `--name WERT` and `--name=WERT` both work; after `--` everything is a free argument.
fn parse(args: &[String], allowed: &[&str]) -> Result<Options, Usage> {
    let mut options = Options::default();
    let mut rest = args.iter();
    let mut only_positional = false;
    while let Some(arg) = rest.next() {
        if only_positional || arg == "-" || !arg.starts_with('-') {
            options.positional.push(arg.clone());
            continue;
        }
        if arg == "--" {
            only_positional = true;
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (arg.as_str(), None),
        };
        let name = if name == "-f" { "--format" } else { name };
        if !allowed.contains(&name) {
            return Err(unsupported(&format!("Den Schalter „{name}“")));
        }
        let value = match inline.or_else(|| rest.next().cloned()) {
            Some(value) if !value.is_empty() => value,
            _ => return Err(Usage(format!("{name} braucht einen Wert"))),
        };
        match name {
            "--root" => options.root = Some(PathBuf::from(value)),
            "--font-path" => options.font_paths.push(PathBuf::from(value)),
            "--format" => options.format = Some(value),
            _ => options.ppi = Some(value),
        }
    }
    // As in Typst: the folders from the environment in addition, after those of the command line.
    if allowed.contains(&"--font-path")
        && let Some(paths) = std::env::var_os("TYPST_FONT_PATHS")
    {
        options.font_paths.extend(std::env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty()));
    }
    Ok(options)
}

fn format_of(word: &str) -> Option<OutputFormat> {
    match word.to_lowercase().as_str() {
        "pdf" => Some(OutputFormat::Pdf),
        "png" => Some(OutputFormat::Png),
        "svg" => Some(OutputFormat::Svg),
        _ => None,
    }
}

fn extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Pdf => "pdf",
        OutputFormat::Png => "png",
        OutputFormat::Svg => "svg",
    }
}

/// A fully checked job of `compile`: what is typeset and where to.
#[derive(Debug)]
struct Compile {
    job: CompileJob,
    output: PathBuf,
}

fn compile_args(args: &[String]) -> Result<Compile, Usage> {
    let options = parse(args, &["--root", "--font-path", "--format", "--ppi"])?;
    let (input, output) = match options.positional.as_slice() {
        [] => return Err(Usage("compile braucht eine Eingabedatei".into())),
        [input] => (input, None),
        [input, output] => (input, Some(output)),
        _ => return Err(Usage("compile nimmt eine Eingabe und höchstens eine Ausgabe".into())),
    };
    if input == "-" {
        return Err(unsupported("Die Eingabe von der Standardeingabe (-)"));
    }
    if output.is_some_and(|output| output == "-") {
        return Err(unsupported("Die Ausgabe auf die Standardausgabe (-)"));
    }
    let output = output.map(PathBuf::from);
    let format = match (&options.format, &output) {
        (Some(word), _) => format_of(word)
            .ok_or_else(|| unsupported(&format!("Das Format „{word}“ (möglich sind pdf, png und svg)")))?,
        (None, Some(path)) => match path.extension().and_then(OsStr::to_str) {
            Some(word) => format_of(word).ok_or_else(|| {
                unsupported(&format!("Die Endung „.{word}“ der Ausgabe (möglich sind .pdf, .png und .svg)"))
            })?,
            None => return Err(Usage("die Ausgabe hat keine Endung; --format pdf|png|svg angeben".into())),
        },
        (None, None) => OutputFormat::Pdf,
    };
    let ppi = match &options.ppi {
        Some(word) => match word.parse::<f32>() {
            Ok(ppi) if ppi.is_finite() && ppi > 0.0 => ppi,
            _ => return Err(Usage(format!("--ppi braucht eine Zahl größer als 0, nicht „{word}“"))),
        },
        None => 144.0,
    };
    let input = PathBuf::from(input);
    let output = output.unwrap_or_else(|| input.with_extension(extension(format)));
    Ok(Compile { job: CompileJob { input, root: options.root, font_paths: options.font_paths, format, ppi }, output })
}

/// The output name of a page: `{p}` page number, `{0p}` with leading zeros, `{t}` number of pages.
fn page_path(template: &Path, page: usize, total: usize) -> PathBuf {
    let width = total.to_string().len();
    let name = template
        .to_string_lossy()
        .replace("{0p}", &format!("{page:0width$}"))
        .replace("{p}", &page.to_string())
        .replace("{t}", &total.to_string());
    PathBuf::from(name)
}

fn has_page_template(path: &Path) -> bool {
    let name = path.to_string_lossy();
    ["{p}", "{0p}", "{t}"].iter().any(|mark| name.contains(mark))
}

fn compile(args: &[String], err: &mut dyn Write) -> i32 {
    let Compile { job, output } = match compile_args(args) {
        Ok(compile) => compile,
        Err(Usage(message)) => return usage(err, &message),
    };
    let result = match compile_file(&job) {
        Ok(result) => result,
        Err(error) => {
            for message in &error.messages {
                let _ = writeln!(err, "{message}");
            }
            return 1;
        }
    };
    for warning in &result.warnings {
        let _ = writeln!(err, "{warning}");
    }
    let total = result.pages.len();
    // First check everything, then write: on an error no file is created.
    let targets: Vec<PathBuf> = if job.format == OutputFormat::Pdf || (total == 1 && !has_page_template(&output)) {
        vec![output]
    } else if has_page_template(&output) {
        (1..=total).map(|page| page_path(&output, page, total)).collect()
    } else {
        let word = extension(job.format);
        let _ = writeln!(
            err,
            "error: {total} Seiten als {word} brauchen {{p}}, {{0p}} oder {{t}} im Namen der Ausgabe (z. B. \
             seite-{{0p}}.{word}); nichts geschrieben"
        );
        return 1;
    };
    for (target, bytes) in targets.iter().zip(&result.pages) {
        if let Err(error) = std::fs::write(target, bytes) {
            let _ = writeln!(err, "{}: error: Ausgabe lässt sich nicht schreiben ({error})", target.display());
            return 1;
        }
    }
    0
}

fn fonts(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let options = match parse(args, &["--font-path"]) {
        Ok(options) => options,
        Err(Usage(message)) => return usage(err, &message),
    };
    if let Some(extra) = options.positional.first() {
        return usage(err, &format!("fonts nimmt keine weitere Angabe („{extra}“)"));
    }
    for name in font_names(&options.font_paths) {
        let _ = writeln!(out, "{name}");
    }
    0
}

fn usage(err: &mut dyn Write, message: &str) -> i32 {
    let _ = writeln!(err, "typst: {message}.\nHilfe: typst --help");
    2
}

/// The Typst command line without the program name; writes only into `out`/`err`, never ends the process.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some((command, rest)) = args.split_first() else {
        let _ = writeln!(err, "{HELP}");
        return 2;
    };
    match command.as_str() {
        "--version" | "-V" => {
            let _ = writeln!(out, "typst {TYPST_VERSION} (Baruch {})", env!("CARGO_PKG_VERSION"));
            0
        }
        "--help" | "-h" | "help" => {
            let _ = writeln!(out, "{HELP}");
            0
        }
        "compile" | "c" => compile(rest, err),
        "fonts" => fonts(rest, out, err),
        name if UNSUPPORTED_COMMANDS.contains(&name) => {
            let Usage(message) = unsupported(&format!("Den Unterbefehl „{name}“"));
            usage(err, &message)
        }
        name if name.starts_with('-') => {
            let Usage(message) = unsupported(&format!("Den Schalter „{name}“"));
            usage(err, &message)
        }
        name => usage(err, &format!("unbekannter Unterbefehl „{name}“")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_is_recognised_by_file_name_only() {
        for argv0 in ["typst", "/usr/bin/baruch-typst", "./baruch-typst", "Baruch-Typst.EXE", "typst.exe"] {
            assert!(is_alias(OsStr::new(argv0)), "{argv0}: der Dateiname ohne Ordner und .exe ist ein Aliasname");
        }
        for argv0 in ["baruch-server", "/opt/typst/baruch-server", "typst-neu", "typst.sh", ""] {
            assert!(
                !is_alias(OsStr::new(argv0)),
                "{argv0}: nur der genaue Dateiname zählt, nicht der Ordner und kein ähnlicher Name – sonst startete \
                 der Server unter /opt/typst/ nie"
            );
        }
    }

    #[test]
    fn dispatch_leaves_every_other_call_to_the_server() {
        let call = |args: &[&str]| dispatch(args.iter().map(OsString::from).collect());
        for args in [&["baruch-server"][..], &["baruch-server", "serve"], &["baruch-server", "import", "typst"], &[]] {
            assert_eq!(
                call(args),
                None,
                "{args:?}: kein Alias und args[1] nicht „typst“ – der Server läuft wie bisher"
            );
        }
        assert_eq!(
            call(&["baruch-server", "typst", "watch", "a.typ"]),
            Some(2),
            "Unterbefehl typst: die Typst-Kommandozeile antwortet (watch gibt es nicht, Exit 2)"
        );
        assert_eq!(
            call(&["/usr/bin/baruch-typst", "watch", "a.typ"]),
            Some(2),
            "Aliasname in argv[0]: dieselbe Antwort ohne das Wort typst"
        );
    }

    #[test]
    fn page_templates_number_the_output_files() {
        let path = |template: &str, page| page_path(Path::new(template), page, 12).to_string_lossy().into_owned();
        assert_eq!(path("seite-{p}.png", 3), "seite-3.png", "{{p}} ist die Seitennummer ab 1");
        assert_eq!(path("seite-{0p}.png", 3), "seite-03.png", "{{0p}} füllt auf die Stellen der Seitenzahl auf");
        assert_eq!(path("{p}-von-{t}.svg", 12), "12-von-12.svg", "{{t}} ist die Seitenzahl");
        assert!(has_page_template(Path::new("a-{t}.png")), "auch {{t}} allein gilt als Vorlage (wie bei Typst)");
        assert!(!has_page_template(Path::new("a-{n}.png")), "ein anderes Wort in Klammern ist keine Vorlage");
    }

    #[test]
    fn the_format_follows_the_flag_then_the_output_name() {
        let format = |list: &[&str]| {
            let args: Vec<String> = list.iter().map(|word| (*word).to_owned()).collect();
            compile_args(&args).map(|c| (c.job.format, c.output)).map_err(|u| u.0)
        };
        assert_eq!(
            format(&["a.typ"]),
            Ok((OutputFormat::Pdf, PathBuf::from("a.pdf"))),
            "ohne Angabe: PDF neben der Eingabe"
        );
        assert_eq!(
            format(&["a.typ", "b.svg"]),
            Ok((OutputFormat::Svg, PathBuf::from("b.svg"))),
            "ohne --format entscheidet die Endung der Ausgabe"
        );
        assert_eq!(
            format(&["-f", "png", "a.typ"]),
            Ok((OutputFormat::Png, PathBuf::from("a.png"))),
            "-f vor der Eingabe; die Ausgabe bekommt die Endung des Formats"
        );
        assert_eq!(
            format(&["a.typ", "b.pdf", "--format=svg"]),
            Ok((OutputFormat::Svg, PathBuf::from("b.pdf"))),
            "--format=… schlägt die Endung; der genannte Name bleibt"
        );
        assert!(
            format(&["a.typ", "b.html"]).unwrap_err().contains("unterstützt Baruch als Typst-Alias nicht"),
            "HTML setzt die eingebaute Kommandozeile nicht – ehrlich abgelehnt statt als PDF geschrieben"
        );
        assert!(
            format(&["a.typ", "--ppi", "0"]).unwrap_err().contains("--ppi"),
            "eine Auflösung von 0 ist ein Aufruffehler"
        );
        assert!(
            format(&["a.typ", "--root"]).unwrap_err().contains("--root braucht einen Wert"),
            "ein Schalter ohne Wert wird genannt"
        );
    }
}
