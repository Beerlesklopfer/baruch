// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Kommandozeile von Baruch (Runde 36, Schnittstelle R36-S1, Besitz `typst_alias`): `baruch-server typst …`
//! oder das Programm unter einem der Namen aus [`ALIAS_NAMES`] gerufen (Symlink `baruch-typst`). Die Engine steht
//! in `baruch_render::cli`, hier stehen die Argumente.
//!
//! Exit-Codes (fest): 0 Erfolg; 1 Satz- oder Dateifehler; 2 falscher Aufruf oder Nicht-Unterstütztes.
//!
//! Stand S1: [`is_alias`] und [`dispatch`] sind fertig, [`run`] ist ein Stub.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;

/// Namen, unter denen das Programm als Typst-Kommandozeile läuft.
pub const ALIAS_NAMES: &[&str] = &["typst", "baruch-typst"];

/// Dateiname von `argv[0]` ohne Ordner und ohne „.exe“ (Groß/Klein egal) ist einer aus [`ALIAS_NAMES`].
pub fn is_alias(argv0: &OsStr) -> bool {
    let Some(name) = Path::new(argv0).file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let name = name.to_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    ALIAS_NAMES.contains(&name)
}

/// `args` samt `argv[0]`. `Some(Exit-Code)`, wenn als Alias gerufen (dann gelten `args[1..]`) oder
/// `args[1] == "typst"` (dann `args[2..]`); sonst `None`.
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

/// Die Typst-Kommandozeile ohne Programmnamen; schreibt nur in `out`/`err`, beendet den Prozess nie.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    // STUB: Typst-Kommandozeile (Runde 36, typst_alias)
    let _ = (args, out);
    let _ = writeln!(err, "typst: in diesem Stand noch nicht umgesetzt (Runde 36).");
    2
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
        assert_eq!(call(&["baruch-server", "typst", "--version"]), Some(2), "Unterbefehl typst: Stub meldet Exit 2");
        assert_eq!(call(&["/usr/bin/baruch-typst", "--version"]), Some(2), "Aliasname in argv[0]: Stub meldet Exit 2");
    }

    #[test]
    fn the_stub_writes_only_to_err() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&["compile".into(), "a.typ".into()], &mut out, &mut err), 2, "Nicht-Unterstütztes ist Exit 2");
        assert!(out.is_empty(), "ein Stub gibt nichts auf der Standardausgabe aus");
        assert!(
            String::from_utf8_lossy(&err).contains("noch nicht umgesetzt"),
            "der Stub antwortet ehrlich (AGENTS.md, Stubs)"
        );
    }
}
