// SPDX-License-Identifier: AGPL-3.0-or-later
//! Concurrent RPC calls `revision.set_current`, `revision.update` and `revision.delete` on the same section (finding
//! from the review of Round 3). Real data folder (SQLite, library, Git mirror) in the temp folder, requests in parallel
//! through the router. Expected: exactly one current revision, no lost change, the current revision never deleted and
//! exactly one commit in the mirror per successful change.

use crate::common;

use std::collections::BTreeSet;

use serde_json::{Value, json};

const SEQ: usize = 2; // Introduction in the sample work: 1 superseded, 2 current, 3 proposal

fn commit_count(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

/// Number of commits with the trailer `Baruch-Action: <action>`.
fn action_count(dir: &std::path::Path, action: &str) -> Option<usize> {
    let log = common::git(dir, &["log", "--format=%(trailers:key=Baruch-Action,valueonly)"])?;
    Some(log.lines().filter(|l| l.trim() == action).count())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_rpc_changes_on_one_section_keep_one_current_and_one_commit_each() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    for n in 4..=12 {
        let saved = server
            .rpc("revision.save", json!({ "node": SEQ, "body": format!("= Einleitung\n\nFassung {n}.\n") }))
            .await;
        assert_eq!(saved["result"]["rev"], n, "Fassung {n} angelegt: {saved}");
    }
    let commits_before = commit_count(&dir);
    let switched_before = action_count(&dir, "switched");

    // Switching to 1–12 (three times), changing 1–6 (once each), deleting 7–11.
    let mut tasks = Vec::new();
    for _ in 0..3 {
        for rev in 1..=12u32 {
            let app = server.app.clone();
            tasks.push(tokio::spawn(async move {
                let answer = common::rpc(app, "revision.set_current", json!({ "node": SEQ, "rev": rev })).await;
                ("set_current", rev, answer)
            }));
        }
    }
    for rev in 1..=6u32 {
        let app = server.app.clone();
        tasks.push(tokio::spawn(async move {
            let body = format!("= Einleitung\n\nParallel geändert: {rev}.\n");
            let answer = common::rpc(app, "revision.update", json!({ "node": SEQ, "rev": rev, "body": body })).await;
            ("update", rev, answer)
        }));
    }
    for rev in 7..=11u32 {
        let app = server.app.clone();
        tasks.push(tokio::spawn(async move {
            let answer = common::rpc(app, "revision.delete", json!({ "node": SEQ, "rev": rev, "confirm": true })).await;
            ("delete", rev, answer)
        }));
    }
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.expect("Aufgabe ist abgestürzt"));
    }

    let mut succeeded = 0usize;
    let mut switches = 0usize;
    let mut deleted = BTreeSet::new();
    let mut updated = Vec::new();
    for (method, rev, answer) in &results {
        if let Some(error) = answer.get("error") {
            // Only domain cases are rejected: already current, currently current (deleting) or deleted.
            let code = error["code"].as_i64().unwrap();
            assert!(code == -32000 || code == -32602, "{method} {rev}: nur fachliche Ablehnung erlaubt: {answer}");
            assert_ne!(*method, "update", "Ändern von {rev} darf nicht scheitern: {answer}");
            continue;
        }
        assert!(answer["result"]["commit"].is_string(), "{method} {rev}: jede Änderung wird gespiegelt: {answer}");
        succeeded += 1;
        match *method {
            "set_current" => switches += 1,
            "update" => updated.push(*rev),
            _ => {
                deleted.insert(*rev);
            }
        }
    }

    let node = server.rpc("node.get", json!({ "node": SEQ })).await["result"].clone();
    assert_eq!(common::currents(&node), 1, "genau eine aktuelle Fassung: {node}");
    let revisions = node["revisions"].as_array().unwrap();
    let current = revisions.iter().find(|r| r["status"] == "current").unwrap()["number"].as_u64().unwrap() as u32;
    assert!(!deleted.contains(&current), "die aktuelle Fassung {current} wurde gelöscht");
    let present: BTreeSet<u32> = revisions.iter().map(|r| r["number"].as_u64().unwrap() as u32).collect();
    for rev in 7..=11 {
        assert_eq!(present.contains(&rev), !deleted.contains(&rev), "Fassung {rev}: Löschen und Bestand passen");
    }
    assert_eq!(updated.len(), 6, "alle sechs Änderungen gelingen: {results:?}");
    for rev in updated {
        let found = revisions.iter().find(|r| r["number"] == rev).unwrap_or_else(|| panic!("Fassung {rev} fehlt"));
        assert_eq!(
            found["body"],
            Value::from(format!("= Einleitung\n\nParallel geändert: {rev}.\n")),
            "Änderung an Fassung {rev} ist verloren gegangen"
        );
    }
    assert!(switches > 0, "mindestens ein Umschalten gelingt");

    let (Some(before), Some(after)) = (commits_before, commit_count(&dir)) else { return };
    assert_eq!(after - before, succeeded as u32, "je erfolgreicher Änderung genau ein Commit");
    // The first successful switch to proposal 3 is an acceptance (without trailer), every further one an ordinary
    // switch.
    let accepted =
        usize::from(results.iter().any(|(m, r, a)| *m == "set_current" && *r == 3 && a.get("result").is_some()));
    assert_eq!(
        action_count(&dir, "switched").unwrap() - switched_before.unwrap(),
        switches - accepted,
        "jede Umschaltung genau ein Commit mit Trailer switched"
    );
    assert_eq!(action_count(&dir, "deleted").unwrap(), deleted.len(), "je Löschung ein Commit mit Trailer deleted");
    assert_eq!(action_count(&dir, "updated").unwrap(), 6, "je Änderung ein Commit mit Trailer updated");
}
