// SPDX-License-Identifier: AGPL-3.0-or-later
//! Übernehmen und Speichern: erst atomar in die Datenbank, danach genau ein Commit im
//! Typst-Spiegel `<data>/works/<slug>/`.

use crate::common;

use baruch_gitmirror::Mirror;
use serde_json::json;

#[tokio::test]
async fn the_mirror_holds_the_database_state_after_start() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let main = std::fs::read_to_string(dir.join("main.typ")).expect("main.typ fehlt im Spiegel");
    assert!(main.starts_with("// Baruch: "), "ohne Werkvorlage beginnt main.typ mit dem Kommentar: {main}");
    assert!(main.contains("#include \"abschnitte/"), "main.typ bindet die Abschnittsdateien ein: {main}");
    assert!(Mirror::open_or_init(&dir).unwrap().head().unwrap().is_some(), "Startstand nicht committet");
    // Ein zweiter Start ändert nichts und committet nicht erneut.
    let head = Mirror::open_or_init(&dir).unwrap().head().unwrap();
    drop(baruch_server::AppState::open(server.path()).await.unwrap());
    assert_eq!(Mirror::open_or_init(&dir).unwrap().head().unwrap(), head, "leerer Commit beim Neustart");
}

#[tokio::test]
async fn accepting_a_proposal_writes_exactly_one_commit_with_trailers() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before = Mirror::open_or_init(&dir).unwrap().head().unwrap();
    let count_before = common::git(&dir, &["rev-list", "--count", "HEAD"]);

    let answer = server.rpc("revision.accept", json!({ "node": 2, "rev": 3 })).await;
    let commit = answer["result"]["commit"].as_str().unwrap_or_else(|| panic!("kein Commit gemeldet: {answer}"));
    assert!(answer["result"].get("mirror").is_none(), "Spiegel meldet Fehler: {answer}");

    // Datenbank: der Vorschlag ist jetzt die einzige aktuelle Revision.
    let node = server.rpc("node.get", json!({ "node": 2 })).await;
    assert_eq!(common::currents(&node["result"]), 1, "nach dem Übernehmen genau eine aktuelle Fassung");
    let body = node["result"]["revisions"][2]["body"].as_str().unwrap().to_owned();

    // Spiegel: HEAD ist der gemeldete Commit, die Datei trägt den übernommenen Text.
    let head = Mirror::open_or_init(&dir).unwrap().head().unwrap();
    assert_ne!(head, before, "kein neuer Commit");
    assert_eq!(head.as_deref(), Some(commit), "die Antwort meldet genau den neuen HEAD");
    let file = std::fs::read_dir(dir.join("abschnitte"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("002-"))
        .expect("Datei für Abschnitt 2 fehlt");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(file).unwrap()).into_owned(),
        body,
        "Spiegel zeigt nicht die übernommene Revision"
    );

    let Some(count_before) = count_before else {
        eprintln!("git fehlt – Anzahl der Commits und Trailer nicht geprüft");
        return;
    };
    let count_after = common::git(&dir, &["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(
        count_after.trim().parse::<u32>().unwrap(),
        count_before.trim().parse::<u32>().unwrap() + 1,
        "genau ein Commit"
    );
    let log = common::git(&dir, &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%s%n%(trailers:only,unfold)"]).unwrap();
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
    assert!(lines[0].ends_with("(@lektor) <lektor@persona.baruch.invalid>"), "Autor ist die Persona: {log}");
    assert_eq!(lines[1], "Autor <autor@baruch.invalid>", "Committer ist der Mensch: {log}");
    assert!(lines[2].ends_with(": Vorschlag 3 übernommen"), "Betreff: {log}");
    for trailer in ["Persona: @lektor", "Baruch-Node: 2", "Baruch-Revision: 3"] {
        assert!(lines.contains(&trailer), "Trailer {trailer} fehlt: {log}");
    }
    assert!(lines.iter().any(|l| l.starts_with("Model: ")), "Trailer Model fehlt: {log}");
    assert_eq!(common::git(&dir, &["status", "--porcelain"]).unwrap(), "", "Spiegel nicht sauber");
}

#[tokio::test]
async fn saving_commits_as_the_human_without_persona_trailers() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let answer = server.rpc("revision.save", json!({ "node": 4, "body": "== Erste Anzeichen\n\nNeu.\n" })).await;
    assert!(answer["result"]["commit"].is_string(), "kein Commit gemeldet: {answer}");
    let Some(log) = common::git(&dir, &["log", "-1", "--format=%an <%ae>%n%s%n%(trailers:only,unfold)"]) else {
        return;
    };
    assert!(log.starts_with("Autor <autor@baruch.invalid>\n"), "Speichern committet als Mensch: {log}");
    assert!(log.contains("Revision") && log.contains("gespeichert"), "Betreff nennt die gespeicherte Revision: {log}");
    assert!(!log.contains("Persona:"), "Mensch ohne Persona-Trailer: {log}");
}

#[tokio::test]
async fn a_broken_mirror_does_not_block_the_work_and_is_reported() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    // Spiegel zerstören: .git durch eine Datei ersetzen.
    std::fs::remove_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".git"), "kaputt").unwrap();
    let answer = server.rpc("revision.accept", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(answer["result"]["rev"], 3, "Übernahme muss trotz Spiegelfehler gelten: {answer}");
    assert!(answer["result"]["mirror"].is_string(), "Spiegelfehler nicht gemeldet: {answer}");
    let node = server.rpc("node.get", json!({ "node": 2 })).await;
    assert_eq!(node["result"]["revisions"][2]["status"], "current", "Datenbank ist die Quelle");
}

/// Spiegeldatei eines Abschnitts (`abschnitte/NNN-…`).
fn section_file(dir: &std::path::Path, seq: usize) -> std::path::PathBuf {
    let prefix = format!("{seq:03}-");
    std::fs::read_dir(dir.join("abschnitte"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(&prefix))
        .unwrap_or_else(|| panic!("Datei für Abschnitt {seq} fehlt"))
}

#[tokio::test]
async fn a_mirror_catches_up_after_an_error() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let head = Mirror::open_or_init(&dir).unwrap().head().unwrap();
    // Spiegel stören: An der Stelle der Datei von Abschnitt 2 steht ein Ordner.
    let blocked = section_file(&dir, 2);
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir(&blocked).unwrap();
    std::fs::write(blocked.join("im-weg"), "x").unwrap();

    let answer = server.rpc("revision.accept", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(answer["result"]["rev"], 3, "Übernahme gilt trotz Spiegelfehler: {answer}");
    assert!(answer["result"]["mirror"].is_string(), "Spiegelfehler nicht gemeldet: {answer}");
    assert!(answer["result"].get("commit").is_none(), "trotz Fehler ein Commit gemeldet: {answer}");
    assert_eq!(Mirror::open_or_init(&dir).unwrap().head().unwrap(), head, "fehlgeschlagener Spiegel hat committet");

    // Reparieren; die nächste Änderung (an einem anderen Abschnitt) bringt den vollen DB-Stand.
    std::fs::remove_dir_all(&blocked).unwrap();
    let body = "== Erste Anzeichen\n\nNach der Reparatur.\n";
    let answer = server.rpc("revision.save", json!({ "node": 4, "body": body })).await;
    let commit = answer["result"]["commit"].as_str().unwrap_or_else(|| panic!("kein Commit nach Reparatur: {answer}"));
    assert!(answer["result"].get("mirror").is_none(), "Spiegel meldet nach Reparatur noch Fehler: {answer}");

    let node2 = server.rpc("node.get", json!({ "node": 2 })).await;
    let accepted = node2["result"]["revisions"][2]["body"].as_str().unwrap();
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 2)).unwrap()).into_owned(),
        accepted,
        "übernommener Vorschlag fehlt im Spiegel"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 4)).unwrap()).into_owned(),
        body,
        "gespeicherte Revision fehlt im Spiegel"
    );
    let Some(files) = changed_files(&dir, commit) else {
        eprintln!("git fehlt – Inhalt des Nachhol-Commits nicht geprüft");
        return;
    };
    assert!(
        files.iter().any(|f| f.starts_with("abschnitte/002-"))
            && files.iter().any(|f| f.starts_with("abschnitte/004-")),
        "der Commit nach der Reparatur muss beide Stände enthalten: {files:?}"
    );
    assert_eq!(common::git(&dir, &["status", "--porcelain"]).unwrap(), "", "Spiegel nach dem Nachholen nicht sauber");
}

/// Dateien, die ein Commit gegenüber seinem Vorgänger ändert.
fn changed_files(dir: &std::path::Path, commit: &str) -> Option<Vec<String>> {
    let out = common::git(dir, &["show", "--name-only", "--format=", commit])?;
    Some(out.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_changes_each_get_their_own_commit() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    for round in 0..8 {
        // Übernehmen (nur in der ersten Runde, Abschnitt 2 hat einen Vorschlag) bzw. Speichern auf
        // Abschnitt 2 und gleichzeitig Speichern auf Abschnitt 4.
        let first = if round == 0 {
            tokio::spawn(common::rpc(server.app.clone(), "revision.accept", json!({ "node": 2, "rev": 3 })))
        } else {
            let body = format!("= Einleitung\n\nRunde {round}.\n");
            tokio::spawn(common::rpc(server.app.clone(), "revision.save", json!({ "node": 2, "body": body })))
        };
        let body = format!("== Erste Anzeichen\n\nRunde {round}.\n");
        let second = tokio::spawn(common::rpc(server.app.clone(), "revision.save", json!({ "node": 4, "body": body })));
        let (a, b) = (first.await.unwrap(), second.await.unwrap());
        let ca = a["result"]["commit"].as_str().unwrap_or_else(|| panic!("Runde {round}: kein Commit für 2: {a}"));
        let cb = b["result"]["commit"].as_str().unwrap_or_else(|| panic!("Runde {round}: kein Commit für 4: {b}"));
        assert_ne!(ca, cb, "Runde {round}: beide Änderungen in einem Commit");
        let Some(files_a) = changed_files(&dir, ca) else { continue };
        let files_b = changed_files(&dir, cb).unwrap();
        assert!(
            files_a.len() == 1 && files_a[0].starts_with("abschnitte/002-"),
            "Runde {round}: Commit für 2 enthält fremden Inhalt: {files_a:?}"
        );
        assert!(
            files_b.len() == 1 && files_b[0].starts_with("abschnitte/004-"),
            "Runde {round}: Commit für 4 enthält fremden Inhalt: {files_b:?}"
        );
        let trailer = common::git(&dir, &["log", "-1", "--format=%(trailers:key=Baruch-Node,valueonly)", ca]).unwrap();
        assert_eq!(trailer.trim(), "2", "Runde {round}: Trailer passt nicht zum Inhalt");
    }
}

/// Deterministisch: Die Schreibsperre wird von außen gehalten, während zwei Änderungen anlaufen.
/// Keine darf vorher in der Datenbank landen (sonst nähme der erste Schnappschuss beide mit), und
/// danach hat jede ihren eigenen Commit mit genau ihrer Datei.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_write_lock_covers_the_database_write() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let held = server.state.write_lock.lock().await;
    let accept = tokio::spawn(common::rpc(server.app.clone(), "revision.accept", json!({ "node": 2, "rev": 3 })));
    let body = "== Erste Anzeichen\n\nGleichzeitig.\n";
    let save = tokio::spawn(common::rpc(server.app.clone(), "revision.save", json!({ "node": 4, "body": body })));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let node2 = server.rpc("node.get", json!({ "node": 2 })).await;
    assert_eq!(node2["result"]["revisions"][2]["status"], "proposed", "Übernehmen schrieb an der Sperre vorbei");
    let node4 = server.rpc("node.get", json!({ "node": 4 })).await;
    assert!(
        node4["result"]["revisions"].as_array().unwrap().iter().all(|r| r["body"] != body),
        "Speichern schrieb an der Sperre vorbei"
    );
    drop(held);
    let (a, b) = (accept.await.unwrap(), save.await.unwrap());
    let ca = a["result"]["commit"].as_str().unwrap_or_else(|| panic!("kein Commit für Übernehmen: {a}"));
    let cb = b["result"]["commit"].as_str().unwrap_or_else(|| panic!("kein Commit für Speichern: {b}"));
    assert_ne!(ca, cb, "zwei Änderungen ergeben zwei verschiedene Commits");
    let Some(files_a) = changed_files(&dir, ca) else { return };
    assert!(
        files_a.len() == 1 && files_a[0].starts_with("abschnitte/002-"),
        "der Commit des Übernehmens enthält nur die Datei der Einleitung: {files_a:?}"
    );
    let files_b = changed_files(&dir, cb).unwrap();
    assert!(
        files_b.len() == 1 && files_b[0].starts_with("abschnitte/004-"),
        "der Commit des Speicherns enthält nur die Datei von 2.1: {files_b:?}"
    );
}

#[tokio::test]
async fn every_outline_change_is_exactly_one_commit_by_the_human() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let dir = server.mirror_dir_of(&report.slug);
    let work = report.work;
    let steps = [
        (
            "outline.move",
            json!({ "node": 3, "target": 0, "place": "before" }),
            "„Sommer“ vor „Frühling“ verschoben",
            "outline-move",
            "0",
        ),
        ("outline.set_level", json!({ "node": 2, "level": 1 }), "„Aussaat“ Ebene 2 → 1", "outline-level", "2"),
        (
            "outline.rename",
            json!({ "node": 2, "title": "Saat" }),
            "„Aussaat“ → „Saat“ umbenannt",
            "outline-rename",
            "2",
        ),
        (
            "outline.add",
            json!({ "target": 0, "place": "into", "title": "Gießplan" }),
            "„Gießplan“ unter „Sommer“ angelegt",
            "outline-add",
            "1",
        ),
        ("outline.delete", json!({ "node": 1, "confirm": true }), "„Gießplan“ gelöscht", "outline-delete", "0"),
        ("outline.set_numbering", json!({ "numbering": "" }), "keine Nummern", "outline-numbering", "0"),
    ];
    for (method, mut params, subject, action, node) in steps {
        params["work"] = json!(work);
        let before = common::git(&dir, &["rev-list", "--count", "HEAD"]);
        let answer = server.rpc(method, params).await;
        assert!(answer["result"]["commit"].is_string(), "{method}: Commit gemeldet: {answer}");
        let Some(before) = before else { continue };
        let after = common::git(&dir, &["rev-list", "--count", "HEAD"]).unwrap();
        assert_eq!(
            after.trim().parse::<u32>().unwrap(),
            before.trim().parse::<u32>().unwrap() + 1,
            "{method}: ein Commit"
        );
        let log =
            common::git(&dir, &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%s%n%(trailers:only,unfold)"]).unwrap();
        let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines[0], "Autor <autor@baruch.invalid>", "{method}: Autor ist der Mensch: {log}");
        assert_eq!(lines[1], "Autor <autor@baruch.invalid>", "{method}: Committer ist der Mensch: {log}");
        assert_eq!(lines[2], format!("Gliederung: {subject}"), "{method}: Betreff");
        assert!(lines.contains(&format!("Baruch-Action: {action}").as_str()), "{method}: Trailer Action: {log}");
        assert!(lines.contains(&format!("Baruch-Node: {node}").as_str()), "{method}: Trailer Node: {log}");
        assert_eq!(common::git(&dir, &["status", "--porcelain"]).unwrap(), "", "{method}: Spiegel sauber");
    }
    let files: Vec<String> = std::fs::read_dir(dir.join("abschnitte"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let mut files = files;
    files.sort();
    assert_eq!(
        files,
        ["000-sommer.typ", "001-fruehling.typ", "002-saat.typ", "003-tiefe.typ"],
        "Dateien nach neuer seq und aufgelöstem Titel benannt"
    );
}
