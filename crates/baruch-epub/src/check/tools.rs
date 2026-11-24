// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fremde Programme der Prüfung (Besitz „epub-pruefung“, Runde 20c): Suche in einem übergebenen PATH-Inhalt (nie
//! der PATH des Prozesses verändert), `xmllint --noout --nonet -` je Datei über die Standardeingabe und ein Lauf mit
//! Zeitgrenze, der das Kind (unter Unix samt seiner Prozessgruppe, unter Windows samt seinem Prozessbaum über
//! `taskkill /T`) bei Ablauf beendet. Nichts wird heruntergeladen.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Dateinamen, unter denen ein Programm `name` liegen kann: unter Windows zuerst mit den üblichen Endungen
/// (`xmllint.exe`, `epubcheck.bat`), überall zuletzt der bloße Name.
fn file_names(name: &str) -> Vec<String> {
    let mut names = Vec::new();
    if cfg!(windows) {
        names.extend([".exe", ".cmd", ".bat"].iter().map(|ext| format!("{name}{ext}")));
    }
    names.push(name.to_owned());
    names
}

/// Ausführbare Datei `name` im ersten Ordner von `path_env`, der sie hat.
pub(crate) fn find_in_path(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path_env = path_env?;
    let names = file_names(name);
    std::env::split_paths(path_env)
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| names.iter().map(move |file| dir.join(file)).collect::<Vec<_>>())
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// Ausgabe eines beendeten Laufs.
pub(crate) struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Führt `command` mit `input` auf der Standardeingabe aus; nach `timeout` wird das Kind beendet (`Err` mit
/// deutscher Meldung). Die Leser der Ausgaben laufen in eigenen Fäden; nach einem Abbruch wird auf sie nicht
/// gewartet (ein Enkel könnte die Rohre noch offen halten).
pub(crate) fn run(mut command: Command, input: Option<&[u8]>, timeout: Duration) -> Result<Output, String> {
    command.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let program = command.get_program().to_string_lossy().into_owned();
    // „Text file busy“ (ETXTBSY): Ein eben geschriebenes Skript kann kurz noch in einem anderen Faden offen sein.
    let mut tries = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            // Nur unter Unix: Die Nummer 26 heißt unter Windows etwas anderes (ERROR_NOT_DOS_DISK).
            Err(e) if cfg!(unix) && e.raw_os_error() == Some(26) && tries < 20 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("{program} lässt sich nicht starten: {e}")),
        }
    };
    let read_all = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(mut pipe) = pipe {
                // Höchstens 4 MiB Ausgabe behalten; der Rest wird gelesen und verworfen.
                let _ = pipe.by_ref().take(4 * 1024 * 1024).read_to_end(&mut buffer);
                let _ = std::io::copy(&mut pipe, &mut std::io::sink());
            }
            String::from_utf8_lossy(&buffer).into_owned()
        })
    };
    let stdout = read_all(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = read_all(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        let input = input.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                kill_group(&mut child);
                return Err(format!("{program} nach {} s abgebrochen (Zeitgrenze).", timeout.as_secs_f64()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => {
                kill_group(&mut child);
                return Err(format!("{program}: {e}"));
            }
        }
    };
    Ok(Output {
        success: status.success(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Kind beenden; unter Unix auch seine Prozessgruppe (Skripte starten Enkel wie `java`), unter Windows der
/// Prozessbaum (`taskkill /T /F`, Bordmittel seit Windows XP; `epubcheck.bat` startet `java` als Enkel). Auf
/// anderen Systemen bleibt es beim Kind allein. Der Windows-Zweig ist nie gelaufen (lokal kein Windows).
fn kill_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg("--")
            .arg(format!("-{}", child.id()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .arg("/T")
            .arg("/F")
            .arg("/PID")
            .arg(child.id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// `xmllint --noout --nonet -` mit `bytes`: `Ok(None)` wohlgeformt, `Ok(Some(meldung))` nicht wohlgeformt,
/// `Err` = xmllint lief nicht (dann prüft quick-xml).
pub(crate) fn xmllint(xmllint: &Path, bytes: &[u8]) -> Result<Option<String>, String> {
    let mut command = Command::new(xmllint);
    command.arg("--noout").arg("--nonet").arg("-");
    let output = run(command, Some(bytes), Duration::from_secs(30))?;
    if output.success {
        return Ok(None);
    }
    let message = output
        .stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.strip_prefix("-:").unwrap_or(l).to_owned())
        .unwrap_or_else(|| "xmllint meldet einen Fehler ohne Text".to_owned());
    Ok(Some(message))
}
