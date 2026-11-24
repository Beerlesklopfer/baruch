// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahmetests für den Git-Spiegel.

use std::path::Path;
use std::process::Command;

use baruch_gitmirror::{CommitRequest, Mirror, MirrorError, MirrorFile, Signature, format_message};

fn file(path: &str, contents: &str) -> MirrorFile {
    MirrorFile { path: path.to_string(), contents: contents.to_string() }
}

fn human() -> Signature {
    Signature { name: "Autor".to_string(), email: "autor@baruch.invalid".to_string() }
}

fn request(files: Vec<MirrorFile>) -> CommitRequest {
    CommitRequest {
        files,
        author: Signature { name: "Lektor (@lektor)".to_string(), email: "lektor@persona.baruch.invalid".to_string() },
        committer: human(),
        summary: "2.1 Erste Anzeichen: Vorschlag 3 übernommen".to_string(),
        body: "Kürzer gefasst.".to_string(),
        trailers: vec![
            ("Baruch-Node".to_string(), "4".to_string()),
            ("Baruch-Revision".to_string(), "3".to_string()),
            ("Persona".to_string(), "@lektor".to_string()),
            ("Model".to_string(), "qwen3:4b".to_string()),
        ],
        allow_empty: false,
    }
}

fn snapshot() -> Vec<MirrorFile> {
    vec![
        file("main.typ", "// Baruch: Beispiel\n#include \"abschnitte/000-a.typ\"\n#include \"abschnitte/001-b.typ\"\n"),
        file("abschnitte/000-a.typ", "= A\n\nDie Liebe ist langmütig.\n"),
        file("abschnitte/001-b.typ", "= B\n\nZweiter Abschnitt.\n"),
    ]
}

/// Pfade aller Blobs im HEAD-Baum (über gix gelesen).
fn head_paths(dir: &Path) -> Vec<String> {
    let repo = gix::open(dir).unwrap();
    let tree = repo.head_tree_id().unwrap().detach();
    let index = repo.index_from_tree(&tree).unwrap();
    let mut paths: Vec<String> = index.entries().iter().map(|e| e.path(&index).to_string()).collect();
    paths.sort();
    paths
}

/// `git status --porcelain`; `None`, wenn kein git-Programm vorhanden ist.
fn git_status(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output();
    match output {
        Ok(out) if out.status.success() => Some(String::from_utf8(out.stdout).unwrap()),
        Ok(out) => panic!("git status fehlgeschlagen: {}", String::from_utf8_lossy(&out.stderr)),
        Err(err) => {
            eprintln!("Hinweis: kein git-Programm ({err}), Prüfung per git status übersprungen");
            None
        }
    }
}

/// Index-Prüfung per gix: Der Index auf der Platte enthält genau die Pfade und Blobs des HEAD-Baums.
fn assert_index_matches_head(dir: &Path) {
    let repo = gix::open(dir).unwrap();
    let tree = repo.head_tree_id().unwrap().detach();
    let expected = repo.index_from_tree(&tree).unwrap();
    let actual = repo.open_index().unwrap();
    let pairs = |index: &gix::index::File| {
        index.entries().iter().map(|e| (e.path(index).to_string(), e.id)).collect::<Vec<_>>()
    };
    assert_eq!(
        pairs(&actual),
        pairs(&expected),
        "der Index auf der Platte enthält genau die Pfade und Blobs des HEAD-Baums"
    );
    // Dateistatus ist gesetzt (sonst müsste git jede Datei neu einlesen).
    assert!(
        actual.entries().iter().all(|e| e.stat.size > 0),
        "Dateistatus gesetzt, sonst liest git jede Datei neu ein"
    );
}

#[test]
fn first_commit_initialises_repo_on_main() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("works").join("beispiel");
    let mirror = Mirror::open_or_init(&dir).unwrap();
    assert!(dir.join(".git").is_dir(), "open_or_init legt ein Repo an");
    assert_eq!(mirror.head().unwrap(), None, "ein neues Repo hat noch keinen Commit");

    let id = mirror.commit(&request(snapshot())).unwrap().expect("erster Commit");
    assert_eq!(id.len(), 40, "Commit-Id als 40 Hex-Zeichen");
    assert_eq!(mirror.head().unwrap().as_deref(), Some(id.as_str()), "HEAD zeigt auf den neuen Commit");

    let repo = gix::open(&dir).unwrap();
    let head_name = repo.head_name().unwrap().expect("HEAD ist symbolisch");
    assert_eq!(head_name.as_bstr(), "refs/heads/main", "der Zweig heißt main");

    for f in snapshot() {
        assert_eq!(
            std::fs::read_to_string(dir.join(&f.path)).unwrap(),
            f.contents,
            "jede Datei liegt mit ihrem Inhalt im Arbeitsordner"
        );
    }
    assert_eq!(
        head_paths(&dir),
        ["abschnitte/000-a.typ", "abschnitte/001-b.typ", "main.typ"],
        "der HEAD-Baum enthält genau die Dateien des Schnappschusses"
    );
    assert_index_matches_head(&dir);
    if let Some(status) = git_status(&dir) {
        assert_eq!(status, "", "git status nicht sauber");
        // Ein echtes git hält die Objekte für gültig und liest die Trailer.
        let fsck =
            Command::new("git").arg("-C").arg(&dir).args(["fsck", "--strict", "--no-progress"]).output().unwrap();
        assert!(
            fsck.status.success(),
            "git fsck hält die Objekte für gültig: {}",
            String::from_utf8_lossy(&fsck.stderr)
        );
        let trailers = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["log", "-1", "--format=%(trailers:key=Persona,valueonly)"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&trailers.stdout).trim(), "@lektor", "git liest den Trailer Persona");
    }
}

#[test]
fn commit_has_separate_author_committer_and_trailers() {
    let tmp = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(tmp.path()).unwrap();
    let req = request(snapshot());
    let id = mirror.commit(&req).unwrap().unwrap();

    let repo = gix::open(tmp.path()).unwrap();
    let commit = repo.find_commit(gix::ObjectId::from_hex(id.as_bytes()).unwrap()).unwrap();
    let author = commit.author().unwrap();
    let committer = commit.committer().unwrap();
    assert_eq!(author.name, "Lektor (@lektor)", "Autor ist die Persona mit Handle");
    assert_eq!(author.email, "lektor@persona.baruch.invalid", "E-Mail der Persona unter persona.baruch.invalid");
    assert_eq!(committer.name, "Autor", "Committer ist der Mensch");
    assert_eq!(committer.email, "autor@baruch.invalid", "E-Mail des Menschen unter baruch.invalid");
    assert_eq!(commit.parent_ids().count(), 0, "der erste Commit hat keinen Vorgänger");

    let message = commit.message_raw().unwrap().to_string();
    let message = message.trim_end_matches('\n');
    assert_eq!(message, format_message(&req.summary, &req.body, &req.trailers), "Nachricht genau wie format_message");
    let lines: Vec<&str> = message.lines().collect();
    assert_eq!(lines[0], "2.1 Erste Anzeichen: Vorschlag 3 übernommen", "Betreff in der ersten Zeile");
    assert_eq!(lines[1], "", "Leerzeile nach dem Betreff");
    assert_eq!(lines[2], "Kürzer gefasst.", "Rumpf in der dritten Zeile");
    assert_eq!(lines[3], "", "Leerzeile vor den Trailern");
    assert_eq!(
        &lines[lines.len() - 2..],
        ["Persona: @lektor", "Model: qwen3:4b"],
        "Persona und Modell als letzte Trailer"
    );
    assert!(lines.contains(&"Baruch-Node: 4"), "Trailer mit dem Abschnitt");
}

#[test]
fn unchanged_snapshot_makes_no_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(tmp.path()).unwrap();
    let first = mirror.commit(&request(snapshot())).unwrap().unwrap();
    // Reihenfolge egal, Inhalt gleich → kein Commit.
    let mut shuffled = snapshot();
    shuffled.reverse();
    assert_eq!(
        mirror.commit(&request(shuffled)).unwrap(),
        None,
        "gleicher Inhalt in anderer Reihenfolge ergibt keinen Commit"
    );
    assert_eq!(mirror.head().unwrap().as_deref(), Some(first.as_str()), "HEAD bleibt der erste Commit");

    // Wieder öffnen (vorhandenes Repo) und erneut: weiterhin kein Commit.
    let reopened = Mirror::open_or_init(tmp.path()).unwrap();
    assert_eq!(
        reopened.commit(&request(snapshot())).unwrap(),
        None,
        "auch nach erneutem Öffnen kein Commit ohne Änderung"
    );
    assert_eq!(reopened.head().unwrap().as_deref(), Some(first.as_str()), "HEAD bleibt der erste Commit");
}

#[test]
fn allow_empty_records_a_commit_with_an_unchanged_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(tmp.path()).unwrap();
    let first = mirror.commit(&request(snapshot())).unwrap().unwrap();
    let mut log = request(snapshot());
    log.allow_empty = true;
    log.summary = "2.1 Erste Anzeichen: Fassung 1 gelöscht".to_string();
    log.body = "Gelöschter Text:\n= A\n".to_string();
    let second = mirror.commit(&log).unwrap().expect("mit allow_empty entsteht ein Commit trotz gleichem Baum");
    assert_ne!(second, first, "neuer Commit, nicht der alte");
    assert_eq!(mirror.head().unwrap().as_deref(), Some(second.as_str()), "HEAD zeigt auf den Protokoll-Commit");

    let repo = gix::open(tmp.path()).unwrap();
    let commit = repo.find_commit(gix::ObjectId::from_hex(second.as_bytes()).unwrap()).unwrap();
    let parent = commit.parent_ids().next().expect("Protokoll-Commit hat einen Vorgänger").detach();
    assert_eq!(parent.to_string(), first, "Vorgänger ist der erste Commit");
    let parent_tree = repo.find_commit(parent).unwrap().tree_id().unwrap().detach();
    assert_eq!(commit.tree_id().unwrap().detach(), parent_tree, "der Baum bleibt unverändert");
    let message = commit.message_raw().unwrap().to_string();
    assert!(message.contains("Fassung 1 gelöscht") && message.contains("Gelöschter Text:"), "Nachricht: {message}");
    if let Some(status) = git_status(tmp.path()) {
        assert_eq!(status, "", "Arbeitsordner bleibt sauber");
    }
}

#[test]
fn missing_file_is_removed_and_history_continues() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mirror = Mirror::open_or_init(dir).unwrap();
    let first = mirror.commit(&request(snapshot())).unwrap().unwrap();

    // Eine fremde, nie verwaltete Datei bleibt unangetastet.
    std::fs::write(dir.join("notiz.txt"), "eigene Notiz").unwrap();

    let mut files = snapshot();
    files.retain(|f| f.path != "abschnitte/001-b.typ");
    files[0].contents = "// Baruch: Beispiel\n#include \"abschnitte/000-a.typ\"\n".to_string();
    files.push(file("anhang/neu/x.typ", "neu\n"));
    let second = mirror.commit(&request(files)).unwrap().unwrap();
    assert_ne!(first, second, "die Änderung ergibt einen neuen Commit");

    assert!(!dir.join("abschnitte/001-b.typ").exists(), "die entfernte Datei ist aus dem Arbeitsordner verschwunden");
    assert!(dir.join("abschnitte/000-a.typ").exists(), "die verbleibende Datei ist noch da");
    assert_eq!(
        std::fs::read_to_string(dir.join("anhang/neu/x.typ")).unwrap(),
        "neu\n",
        "die neue Datei in einem neuen Unterordner ist geschrieben"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("notiz.txt")).unwrap(),
        "eigene Notiz",
        "eine fremde, nie verwaltete Datei bleibt unangetastet"
    );
    assert_eq!(
        head_paths(dir),
        ["abschnitte/000-a.typ", "anhang/neu/x.typ", "main.typ"],
        "der HEAD-Baum folgt dem neuen Schnappschuss"
    );

    let repo = gix::open(dir).unwrap();
    let commit = repo.find_commit(gix::ObjectId::from_hex(second.as_bytes()).unwrap()).unwrap();
    let parents: Vec<String> = commit.parent_ids().map(|p| p.to_string()).collect();
    assert_eq!(parents, [first], "die Historie geht weiter: Vorgänger ist der erste Commit");

    // Leerer Unterordner verschwindet, wenn seine letzte Datei geht.
    let only_main = vec![file("main.typ", "// leer\n")];
    mirror.commit(&request(only_main)).unwrap().unwrap();
    assert!(!dir.join("abschnitte").exists(), "leerer Unterordner abschnitte verschwindet");
    assert!(!dir.join("anhang").exists(), "leerer Unterordner anhang verschwindet");
    assert_eq!(head_paths(dir), ["main.typ"], "im HEAD-Baum bleibt nur main.typ");
    assert_index_matches_head(dir);
    if let Some(status) = git_status(dir) {
        assert_eq!(status, "?? notiz.txt\n", "nur die fremde Datei ist unversioniert");
    }
}

#[test]
fn status_clean_after_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(tmp.path()).unwrap();
    mirror.commit(&request(snapshot())).unwrap().unwrap();
    let mut files = snapshot();
    files[1].contents = "= A\n\nDie Liebe ist langmütig und freundlich.\n".to_string();
    mirror.commit(&request(files)).unwrap().unwrap();
    assert_index_matches_head(tmp.path());
    if let Some(status) = git_status(tmp.path()) {
        assert_eq!(status, "", "git status nicht sauber");
    }
}

#[test]
fn invalid_paths_write_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("spiegel");
    let mirror = Mirror::open_or_init(&dir).unwrap();
    for bad in ["/etc/passwd", "../aussen.typ", "a/../../b", ".git/config", "", "a\\b.typ"] {
        let mut files = snapshot();
        files.push(file(bad, "böse"));
        match mirror.commit(&request(files)) {
            Err(MirrorError::InvalidPath(p)) => assert_eq!(p, bad, "die Meldung nennt den abgelehnten Pfad"),
            other => panic!("{bad:?}: erwartet InvalidPath, bekam {other:?}"),
        }
    }
    assert_eq!(mirror.head().unwrap(), None, "kein Commit nach ungültigen Pfaden");
    let mut entries: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    entries.sort();
    assert_eq!(entries, [".git"], "nichts geschrieben");
    assert!(!tmp.path().join("aussen.typ").exists(), "nichts außerhalb des Spiegels geschrieben");
}

#[test]
fn format_message_variants() {
    assert_eq!(format_message("Nur Betreff", "", &[]), "Nur Betreff", "nur Betreff ohne Leerzeilen");
    let trailers = vec![("Persona".to_string(), "@lektor".to_string())];
    assert_eq!(
        format_message("S", "B", &trailers),
        "S\n\nB\n\nPersona: @lektor",
        "Betreff, Rumpf und Trailer durch Leerzeilen getrennt"
    );
}

#[test]
fn mirror_can_move_between_threads() {
    // Der Server hält den Spiegel in einem Arc und committet in `spawn_blocking`.
    fn send_sync<T: Send + Sync + 'static>() {}
    send_sync::<baruch_gitmirror::Mirror>();
}

/// Ordnerinhalt (Dateinamen, sortiert) für Vorher/Nachher-Vergleiche.
#[cfg(unix)]
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[cfg(unix)]
#[test]
fn a_symlinked_section_folder_is_refused_and_nothing_is_written_outside() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("spiegel");
    let outside = root.path().join("fremd");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("000-a.typ"), "fremd\n").unwrap();
    let mirror = Mirror::open_or_init(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("abschnitte")).unwrap();

    let result = mirror.commit(&request(snapshot()));
    assert!(
        matches!(result, Err(MirrorError::InvalidPath(ref p)) if p.starts_with("abschnitte/")),
        "Symlink als Zwischenordner muss abgelehnt werden: {result:?}"
    );
    assert_eq!(listing(&outside), vec!["000-a.typ"], "außerhalb des Spiegels darf keine Datei entstehen");
    assert_eq!(
        std::fs::read_to_string(outside.join("000-a.typ")).unwrap(),
        "fremd\n",
        "eine fremde Datei darf nicht überschrieben werden"
    );
    assert!(!dir.join("main.typ").exists(), "vor der Prüfung aller Pfade darf nichts geschrieben werden");
    assert_eq!(mirror.head().unwrap(), None, "kein Commit trotz Ablehnung");
}

#[cfg(unix)]
#[test]
fn removing_a_stale_file_does_not_follow_a_symlinked_folder() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("spiegel");
    let outside = root.path().join("fremd");
    std::fs::create_dir(&outside).unwrap();
    let mirror = Mirror::open_or_init(&dir).unwrap();
    mirror.commit(&request(snapshot())).unwrap().expect("erster Commit");
    // Ordner abschnitte durch einen Symlink auf einen fremden Ordner ersetzen, in dem eine Datei
    // mit dem Namen einer jetzt veralteten Spiegeldatei liegt.
    std::fs::remove_dir_all(dir.join("abschnitte")).unwrap();
    std::fs::write(outside.join("001-b.typ"), "fremd\n").unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("abschnitte")).unwrap();
    let head = mirror.head().unwrap();

    let only_main = vec![file("main.typ", "// Baruch: Beispiel\n")];
    let result = mirror.commit(&request(only_main));
    assert!(
        matches!(result, Err(MirrorError::InvalidPath(_))),
        "Löschen über einen Symlink muss abgelehnt werden: {result:?}"
    );
    assert_eq!(
        std::fs::read_to_string(outside.join("001-b.typ")).unwrap(),
        "fremd\n",
        "eine Datei außerhalb des Spiegels darf nicht gelöscht werden"
    );
    assert!(
        std::fs::read_to_string(dir.join("main.typ")).unwrap().contains("#include"),
        "main.typ darf vor der Ablehnung nicht umgeschrieben werden"
    );
    assert_eq!(mirror.head().unwrap(), head, "kein neuer Commit trotz Ablehnung");
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_replaced_without_touching_its_target() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("spiegel");
    let target = root.path().join("ziel.typ");
    std::fs::write(&target, "fremd\n").unwrap();
    let mirror = Mirror::open_or_init(&dir).unwrap();
    std::os::unix::fs::symlink(&target, dir.join("main.typ")).unwrap();
    mirror.commit(&request(snapshot())).unwrap().expect("Commit trotz Symlink an Stelle einer Datei");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "fremd\n", "das Ziel des Symlinks darf sich nicht ändern");
    assert!(
        !std::fs::symlink_metadata(dir.join("main.typ")).unwrap().file_type().is_symlink(),
        "main.typ muss jetzt eine echte Datei im Spiegel sein"
    );
}

// ---- Seitenzweige (Runde 22, Vertrag §2.12) ------------------------------------------------------------------

fn side(name: &str, path: &str, contents: &str) -> baruch_gitmirror::SideRef {
    baruch_gitmirror::SideRef {
        name: name.to_string(),
        path: path.to_string(),
        contents: contents.to_string(),
        author: human(),
        committer: human(),
        summary: format!("Seitenzweig {name}"),
        body: String::new(),
        trailers: vec![("Baruch-Side".to_string(), "superseded".to_string())],
    }
}

#[test]
fn side_refs_hold_the_snapshot_with_one_section_replaced_and_head_as_parent() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(dir.path()).unwrap();
    let head = mirror.commit(&request(snapshot())).unwrap().expect("erster Commit");
    let old = "= A\n\nDie Liebe ist geduldig.\n";
    let sync = mirror
        .sync_side_refs("refs/revisions/", &snapshot(), &[side("refs/revisions/7/1", "abschnitte/000-a.typ", old)])
        .unwrap();
    assert_eq!(sync, baruch_gitmirror::SideSync { written: 1, kept: 0, removed: 0 }, "ein neuer Seitenzweig");
    let refs = mirror.side_refs("refs/revisions/").unwrap();
    assert_eq!(refs.keys().collect::<Vec<_>>(), ["refs/revisions/7/1"], "Referenz unter refs/revisions/<key>/<n>");
    assert_eq!(
        mirror.read_file("refs/revisions/7/1", "abschnitte/000-a.typ").unwrap().as_deref(),
        Some(old),
        "der Abschnitt in dieser Fassung"
    );
    assert_eq!(
        mirror.read_file("refs/revisions/7/1", "abschnitte/001-b.typ").unwrap().as_deref(),
        Some("= B\n\nZweiter Abschnitt.\n"),
        "übrige Dateien wie im Schnappschuss"
    );
    assert_eq!(mirror.parents("refs/revisions/7/1").unwrap(), Some(vec![head.clone()]), "Elternteil ist HEAD");
    assert_eq!(mirror.head().unwrap(), Some(head), "HEAD bleibt");
    assert_index_matches_head(dir.path());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("abschnitte/000-a.typ")).unwrap(),
        "= A\n\nDie Liebe ist langmütig.\n",
        "Arbeitsordner unberührt (zeigt die aktuelle Fassung)"
    );
    if let Some(status) = git_status(dir.path()) {
        assert_eq!(status, "", "git status bleibt sauber");
    }
}

#[test]
fn unchanged_side_refs_are_kept_changed_ones_rewritten_and_missing_ones_removed() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(dir.path()).unwrap();
    mirror.commit(&request(snapshot())).unwrap();
    let a1 = side("refs/revisions/7/1", "abschnitte/000-a.typ", "= A\n\nEins.\n");
    let b2 = side("refs/revisions/8/2", "abschnitte/001-b.typ", "= B\n\nZwei.\n");
    mirror.sync_side_refs("refs/revisions/", &snapshot(), &[a1.clone(), b2.clone()]).unwrap();
    let first = mirror.side_refs("refs/revisions/").unwrap();

    // Neuer HEAD, gleiche Seitenzweige: nichts neu geschrieben (kein Objekt-Zuwachs bei jedem Speichern).
    let mut next = snapshot();
    next[2].contents = "= B\n\nZweiter Abschnitt, neu.\n".into();
    mirror.commit(&request(next.clone())).unwrap();
    let sync = mirror.sync_side_refs("refs/revisions/", &next, &[a1.clone(), b2.clone()]).unwrap();
    assert_eq!(sync, baruch_gitmirror::SideSync { written: 0, kept: 2, removed: 0 }, "beide unverändert behalten");
    assert_eq!(mirror.side_refs("refs/revisions/").unwrap(), first, "gleiche Commits");

    // Text einer Fassung geändert, eine Fassung fort: neu geschrieben bzw. entfernt.
    let a1_changed = side("refs/revisions/7/1", "abschnitte/000-a.typ", "= A\n\nEins, geändert.\n");
    let sync = mirror.sync_side_refs("refs/revisions/", &next, &[a1_changed]).unwrap();
    assert_eq!(sync, baruch_gitmirror::SideSync { written: 1, kept: 0, removed: 1 }, "einer neu, einer entfernt");
    let now = mirror.side_refs("refs/revisions/").unwrap();
    assert_eq!(now.keys().collect::<Vec<_>>(), ["refs/revisions/7/1"], "8/2 entfernt");
    assert_ne!(now["refs/revisions/7/1"], first["refs/revisions/7/1"], "7/1 zeigt auf einen neuen Commit");
    assert_eq!(
        mirror.parents("refs/revisions/7/1").unwrap(),
        Some(vec![mirror.head().unwrap().unwrap()]),
        "neuer Commit hängt am jetzigen HEAD"
    );
    let all = mirror.sync_side_refs("refs/revisions/", &next, &[]).unwrap();
    assert_eq!(all.removed, 1, "ohne Seitenzweige werden alle entfernt");
    assert!(mirror.side_refs("refs/revisions/").unwrap().is_empty(), "keine Referenz übrig");
}

#[test]
fn side_refs_refuse_bad_names_and_paths_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = Mirror::open_or_init(dir.path()).unwrap();
    mirror.commit(&request(snapshot())).unwrap();
    for (name, path) in [
        ("refs/heads/main", "abschnitte/000-a.typ"),
        ("refs/revisions/../heads/x", "abschnitte/000-a.typ"),
        ("refs/revisions/7/1 x", "abschnitte/000-a.typ"),
        ("refs/revisions/", "abschnitte/000-a.typ"),
        ("refs/revisions/7/1", "../draußen.typ"),
        ("refs/revisions/7/1", ".git/config"),
    ] {
        let refused = mirror.sync_side_refs("refs/revisions/", &snapshot(), &[side(name, path, "x")]);
        assert!(matches!(refused, Err(MirrorError::InvalidPath(_))), "abgelehnt: {name} {path}");
    }
    assert!(mirror.side_refs("refs/revisions/").unwrap().is_empty(), "nichts geschrieben");
    let outside = mirror.sync_side_refs("heads/", &snapshot(), &[]);
    assert!(matches!(outside, Err(MirrorError::InvalidPath(_))), "Präfix muss unter refs/ liegen");
}
