// SPDX-License-Identifier: AGPL-3.0-or-later
//! External programs of the check (ownership "epub-pruefung", Round 20c): search in a passed PATH content (the
//! PATH of the process is never changed), `xmllint --noout --nonet -` per file through standard input and a run
//! with a time limit that terminates the child (under Unix including its process group, under Windows including
//! its process tree through `taskkill /T`) on expiry. Nothing is downloaded.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// File names under which a program `name` can be found: under Windows first with the usual extensions
/// (`xmllint.exe`, `epubcheck.bat`), everywhere last the bare name.
fn file_names(name: &str) -> Vec<String> {
    let mut names = Vec::new();
    if cfg!(windows) {
        names.extend([".exe", ".cmd", ".bat"].iter().map(|ext| format!("{name}{ext}")));
    }
    names.push(name.to_owned());
    names
}

/// Executable file `name` in the first folder of `path_env` that has it.
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

/// Output of a finished run.
pub(crate) struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `command` with `input` on standard input; after `timeout` the child is terminated (`Err` with a German
/// message). The readers of the outputs run in threads of their own; after an abort they are not waited for (a
/// grandchild could still hold the pipes open).
pub(crate) fn run(mut command: Command, input: Option<&[u8]>, timeout: Duration) -> Result<Output, String> {
    command.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let program = command.get_program().to_string_lossy().into_owned();
    // "Text file busy" (ETXTBSY): a script that was just written can still be open in another thread for a moment.
    let mut tries = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            // Under Unix only: the number 26 means something else under Windows (ERROR_NOT_DOS_DISK).
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
                // Keep at most 4 MiB of output; the rest is read and discarded.
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

/// Terminate the child; under Unix also its process group (scripts start grandchildren such as `java`), under
/// Windows the process tree (`taskkill /T /F`, on board since Windows XP; `epubcheck.bat` starts `java` as a
/// grandchild). On other systems only the child itself. The Windows branch has never run (no Windows locally).
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

/// `xmllint --noout --nonet -` with `bytes`: `Ok(None)` well-formed, `Ok(Some(meldung))` not well-formed,
/// `Err` = xmllint did not run (then quick-xml checks).
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
