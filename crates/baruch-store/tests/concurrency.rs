// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simultaneous changes to the same section (finding from the review of Round 3): switching, changing and deleting
//! each run in a `BEGIN IMMEDIATE` transaction of their own. Many parallel calls must afterwards never leave two or
//! no current revision, lose no change and never delete the current revision. Real SQLite file (WAL, several
//! connections) in the temp folder.

use std::collections::BTreeSet;

use baruch_core::RevisionStatus;
use baruch_store::{RevisionEdit, Store, StoreError};

const SEQ: usize = 2; // Einleitung in the sample work: 1 superseded, 2 current, 3 proposal

/// What a call has done.
#[derive(Debug)]
enum Outcome {
    Switched(u32),
    Updated(u32, String),
    Deleted(u32),
    Refused,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_switching_editing_and_deleting_keeps_exactly_one_current_and_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    // Revisions 4–12 in addition; afterwards 12 is current.
    for n in 4..=12 {
        store.save_human(work, SEQ, &format!("= Einleitung\n\nFassung {n}.\n"), "Autor").await.unwrap().unwrap();
    }
    // Changing only on 1–6 (each exactly once, so that every change stays verifiable), deleting on 7–11, switching
    // to all numbers 1–12 (also to those that are being deleted right now).
    let mut tasks = Vec::new();
    for round in 0..3u32 {
        for rev in 1..=12u32 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                match store.set_current(work, SEQ, rev).await {
                    Ok(change) => Outcome::Switched(change.rev),
                    Err(StoreError::Refused(_) | StoreError::UnknownRevision { .. }) => Outcome::Refused,
                    Err(e) => panic!("Umschalten auf {rev} (Runde {round}): unerwarteter Fehler {e}"),
                }
            }));
        }
    }
    for rev in 1..=6u32 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let body = format!("= Einleitung\n\nGleichzeitig geändert: {rev}.\n");
            let edit = RevisionEdit { body: Some(&body), note: Some("parallel") };
            match store.update_revision(work, SEQ, rev, &edit, "Autor").await {
                Ok(Some(change)) => Outcome::Updated(change.rev, body),
                Ok(None) => panic!("Fassung {rev}: Änderung ohne Wirkung"),
                Err(e) => panic!("Ändern von {rev}: unerwarteter Fehler {e}"),
            }
        }));
    }
    for rev in 7..=11u32 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            match store.delete_revision(work, SEQ, rev, "Autor").await {
                Ok(change) => Outcome::Deleted(change.rev),
                // Current right now → refused; that is the protected rule.
                Err(StoreError::Refused(_)) => Outcome::Refused,
                Err(e) => panic!("Löschen von {rev}: unerwarteter Fehler {e}"),
            }
        }));
    }
    let mut outcomes = Vec::new();
    for task in tasks {
        outcomes.push(task.await.expect("Aufgabe ist abgestürzt"));
    }

    let loaded = store.load_work(work).await.unwrap();
    let node = &loaded.nodes[SEQ];
    let currents: Vec<u32> =
        node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).map(|r| r.number).collect();
    assert_eq!(currents.len(), 1, "genau eine aktuelle Fassung nach allen parallelen Aufrufen: {currents:?}");
    let present: BTreeSet<u32> = node.revisions.iter().map(|r| r.number).collect();
    let deleted: BTreeSet<u32> =
        outcomes.iter().filter_map(|o| if let Outcome::Deleted(n) = o { Some(*n) } else { None }).collect();
    assert!(!deleted.contains(&currents[0]), "die aktuelle Fassung {} wurde gelöscht: {outcomes:?}", currents[0]);
    for rev in 7..=11 {
        assert_eq!(
            present.contains(&rev),
            !deleted.contains(&rev),
            "Fassung {rev}: gelöscht genau dann, wenn das Löschen gemeldet wurde ({outcomes:?})"
        );
    }
    for outcome in &outcomes {
        if let Outcome::Updated(rev, body) = outcome {
            let found = node.revision(*rev).unwrap_or_else(|| panic!("geänderte Fassung {rev} fehlt"));
            assert_eq!(&found.body, body, "Änderung an Fassung {rev} ist verloren gegangen");
            assert_eq!(found.note, "parallel", "Notiz der Fassung {rev} ist verloren gegangen");
        }
    }
    let updated = outcomes.iter().filter(|o| matches!(o, Outcome::Updated(..))).count();
    assert_eq!(updated, 6, "jede Änderung an 1–6 ist gelungen: {outcomes:?}");
    let switched: Vec<u32> =
        outcomes.iter().filter_map(|o| if let Outcome::Switched(n) = o { Some(*n) } else { None }).collect();
    assert!(!switched.is_empty(), "mindestens ein Umschalten gelingt: {outcomes:?}");
    assert!(switched.iter().all(|n| (1..=12).contains(n)), "umgeschaltet nur auf gewünschte Nummern: {switched:?}");
    let refused = outcomes.iter().filter(|o| matches!(o, Outcome::Refused)).count();
    assert_eq!(switched.len() + refused + updated + deleted.len(), outcomes.len(), "jeder Aufruf hat ein Ergebnis");
    // The numbers stay unique and are not assigned again.
    let saved = store.save_human(work, SEQ, "= Einleitung\n\nDanach.\n", "Autor").await.unwrap().unwrap();
    assert_eq!(saved.rev, 13, "nach parallelem Löschen bekommt die nächste Fassung eine neue Nummer");
}
