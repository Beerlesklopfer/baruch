// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werkweites Ersetzen und Rückgängig im Store (Runde 30a, Migration 0047): eine Transaktion, je Abschnitt eine neue
//! Fassung vom Menschen, Überspringen mit Grund, Rückgängig per Umschalten. Echte SQLite-Datei im Temp-Ordner,
//! Testtexte frei erfunden.

use baruch_core::{Author, RevisionStatus};
use baruch_store::{ReplaceEdit, ReplaceItem, Store, StoreError};
use serde_json::json;

/// Werk mit `n` Abschnitten „Turm i“ unter einem Kapitel, je mit „Leuchtturm“ im Text.
async fn towers(n: usize) -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let mut source = String::from("= Küste\n\nDie Küste ist lang.\n\n");
    for i in 0..n {
        source.push_str(&format!("== Turm {i}\n\nDer Leuchtturm {i} blinkt.\n\n"));
    }
    let work = store.import_typst(&source, "kueste.typ", "Ida").await.unwrap().work;
    (dir, store, work)
}

/// (Schlüssel, aktuelle Nummer, aktueller Text) je Abschnitt.
async fn currents(store: &Store, work: i64) -> Vec<(i64, u32, String)> {
    let loaded = store.load_work(work).await.unwrap();
    loaded
        .nodes
        .iter()
        .map(|n| {
            let rev = n.current_number().unwrap();
            (n.key, rev, n.revision(rev).unwrap().body.clone())
        })
        .collect()
}

#[tokio::test]
async fn a_logged_batch_is_read_back_and_foreign_batches_are_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst("= Leuchtturm\n\nDer Leuchtturm blinkt.\n", "turm.typ", "Ida").await.unwrap().work;
    let items = r#"[{"key": 5, "prev_rev": 1, "new_rev": 2}]"#;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO replace_batch (work_id, query, replacement, items) VALUES (?, 'Leuchtturm', 'Wachturm', ?) \
         RETURNING id",
    )
    .bind(work)
    .bind(items)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let batch = store.replace_batch(work, id).await.unwrap();
    assert_eq!((batch.query.as_str(), batch.replacement.as_str()), ("Leuchtturm", "Wachturm"), "Suchtext und Ersatz");
    assert_eq!(batch.items, vec![ReplaceItem { key: 5, prev_rev: 1, new_rev: 2 }], "Einträge aus JSON");
    assert_eq!(batch.undone, None, "noch nicht rückgängig gemacht");
    assert!(batch.created.ends_with('Z'), "Zeitpunkt in UTC: {}", batch.created);

    let other = store.default_work().await.unwrap();
    assert!(matches!(store.replace_batch(other, id).await, Err(StoreError::Replace(_))), "fremdes Werk: unbekannt");
    assert!(matches!(store.replace_batch(999_999, id).await, Err(StoreError::UnknownWork(_))), "unbekanntes Werk");
}

#[tokio::test]
async fn replacing_in_fifty_sections_makes_fifty_human_revisions_in_one_batch() {
    let (_dir, store, work) = towers(50).await;
    let before = currents(&store, work).await;
    let bodies: Vec<(i64, u32, String)> =
        before.iter().skip(1).map(|(k, r, b)| (*k, *r, b.replace("Leuchtturm", "Wachturm"))).collect();
    let edits: Vec<ReplaceEdit> =
        bodies.iter().map(|(key, rev, body)| ReplaceEdit { key: *key, base_rev: *rev, body }).collect();
    let (batch, changes, skipped) =
        store.replace_apply(work, "Leuchtturm", "Wachturm", &json!({"case": true}), &edits, "Ida").await.unwrap();
    assert_eq!(skipped, vec![], "nichts übersprungen");
    assert_eq!(batch.items.len(), 50, "50 Einträge im Protokoll");
    assert_eq!(changes.len(), 50, "50 Änderungen für den Spiegel");
    assert!(changes.iter().all(|c| c.author == Author::Human { name: "Ida".into() }), "alle vom Menschen");

    let after = currents(&store, work).await;
    assert_eq!(after[0], before[0], "das Kapitel ohne Ersetzung bleibt");
    for ((key, rev, body), item) in after.iter().skip(1).zip(&batch.items) {
        assert_eq!(item.key, *key, "Reihenfolge der Einträge = Reihenfolge der Abschnitte");
        assert_eq!((item.prev_rev, item.new_rev), (1, *rev), "neue Fassung auf der Basis 1");
        assert!(body.contains("Wachturm") && !body.contains("Leuchtturm"), "ersetzt: {body}");
    }
    let loaded = store.load_work(work).await.unwrap();
    let node = &loaded.nodes[1];
    let based_on: Option<i64> = sqlx::query_scalar("SELECT based_on FROM revision WHERE node_id = ? AND number = ?")
        .bind(node.key)
        .bind(i64::from(node.current_number().unwrap()))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(based_on, Some(1), "Elternteil im Revisionsbaum ist die Basis");
    assert_eq!(node.revision(1).unwrap().status, RevisionStatus::Superseded, "die Basis bleibt erhalten");

    let logged = store.replace_batch(work, batch.id).await.unwrap();
    assert_eq!(logged, batch, "das Protokoll liest sich zurück, wie es geschrieben wurde");
    let options: String = sqlx::query_scalar("SELECT options FROM replace_batch WHERE id = ?")
        .bind(batch.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(options, r#"{"case":true}"#, "Optionen stehen im Protokoll");
}

#[tokio::test]
async fn a_section_changed_in_between_is_skipped_with_a_reason() {
    let (_dir, store, work) = towers(2).await;
    let before = currents(&store, work).await;
    let (key, _, body) = &before[1];
    let node = store.node_seq(work, *key).await.unwrap();
    store.save_human(work, node, &format!("{body}Neu dazu.\n"), "Ida").await.unwrap();
    let new_body = body.replace("Leuchtturm", "Wachturm");
    let other = before[2].2.replace("Leuchtturm", "Wachturm");
    let edits = [
        ReplaceEdit { key: *key, base_rev: 1, body: &new_body },
        ReplaceEdit { key: before[2].0, base_rev: 1, body: &other },
        ReplaceEdit { key: 999_999, base_rev: 1, body: "x" },
    ];
    let (batch, _, skipped) =
        store.replace_apply(work, "Leuchtturm", "Wachturm", &json!({}), &edits, "Ida").await.unwrap();
    assert_eq!(batch.items.len(), 1, "nur der unveränderte Abschnitt wird ersetzt");
    assert_eq!(skipped.len(), 2, "geänderter und unbekannter Abschnitt übersprungen: {skipped:?}");
    assert_eq!(skipped[0].key, *key, "der geänderte Abschnitt");
    assert!(skipped[0].reason.contains("inzwischen geändert"), "Grund nennt die Änderung: {}", skipped[0].reason);
    assert!(skipped[1].reason.contains("nicht"), "unbekannter Schlüssel: {}", skipped[1].reason);
    let after = currents(&store, work).await;
    assert!(after[1].2.contains("Leuchtturm") && after[1].2.contains("Neu dazu."), "geänderter Text unangetastet");
}

#[tokio::test]
async fn headings_follow_the_title_but_never_change_their_level() {
    let (_dir, store, work) = towers(2).await;
    let before = currents(&store, work).await;
    let renamed = before[1].2.replace("Turm 0", "Wachturm 0");
    let relevel = before[2].2.replace("== Turm 1", "= Turm 1");
    let edits = [
        ReplaceEdit { key: before[1].0, base_rev: 1, body: &renamed },
        ReplaceEdit { key: before[2].0, base_rev: 1, body: &relevel },
    ];
    let (batch, _, skipped) = store.replace_apply(work, "Turm", "x", &json!({}), &edits, "Ida").await.unwrap();
    assert_eq!(batch.items.len(), 1, "nur die Umbenennung gilt");
    assert_eq!(skipped.len(), 1, "die andere Ebene wird übersprungen");
    assert!(skipped[0].reason.contains("Ebene"), "Grund nennt die Ebene: {}", skipped[0].reason);
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(loaded.nodes[1].title, "Wachturm 0", "der Titel des Gliederungspunkts folgt wie beim Speichern");
    assert_eq!(loaded.nodes[2].title, "Turm 1", "der übersprungene Punkt bleibt");
    assert_eq!(
        loaded.nodes[2].current_number(),
        Some(1),
        "und hat keine neue Fassung (nur sein Schritt zurückgerollt)"
    );
}

#[tokio::test]
async fn undo_switches_back_and_skips_sections_changed_afterwards() {
    let (_dir, store, work) = towers(3).await;
    let before = currents(&store, work).await;
    let bodies: Vec<String> = before.iter().map(|(_, _, b)| b.replace("Leuchtturm", "Wachturm")).collect();
    let edits: Vec<ReplaceEdit> = before
        .iter()
        .zip(&bodies)
        .skip(1)
        .map(|((k, r, _), b)| ReplaceEdit { key: *k, base_rev: *r, body: b })
        .collect();
    let (batch, _, _) = store.replace_apply(work, "Leuchtturm", "Wachturm", &json!({}), &edits, "Ida").await.unwrap();
    assert_eq!(store.replace_latest(work).await.unwrap().map(|b| b.id), Some(batch.id), "jüngstes Ersetzen");
    // Der dritte Abschnitt wird nach dem Ersetzen noch einmal gespeichert.
    let changed = currents(&store, work).await;
    let node = store.node_seq(work, changed[3].0).await.unwrap();
    store.save_human(work, node, &format!("{}Später.\n", changed[3].2), "Ida").await.unwrap();

    let (undone, skipped) = store.replace_undo(work, batch.id, "Ida").await.unwrap();
    assert!(undone.undone.is_some(), "rückgängig gemacht vermerkt");
    assert_eq!(skipped.len(), 1, "der später geänderte Abschnitt wird übersprungen: {skipped:?}");
    assert_eq!(skipped[0].key, changed[3].0, "genau er");
    let after = currents(&store, work).await;
    assert_eq!((after[1].1, after[1].2.as_str()), (1, before[1].2.as_str()), "zurück auf Fassung 1 (Umschalten)");
    assert_eq!(after[2].1, 1, "ebenso");
    assert!(after[3].2.contains("Später."), "der geänderte bleibt");
    let loaded = store.load_work(work).await.unwrap();
    assert!(loaded.nodes[1].revision(2).is_some(), "die ersetzte Fassung bleibt im Baum (nichts gelöscht)");

    let again = store.replace_undo(work, batch.id, "Ida").await.unwrap_err();
    assert!(again.to_string().contains("schon rückgängig"), "zweimal geht nicht: {again}");
    assert_eq!(store.replace_latest(work).await.unwrap(), None, "kein offenes Ersetzen mehr");
}

#[tokio::test]
async fn limits_are_checked_before_anything_is_written() {
    let (_dir, store, work) = towers(1).await;
    let long = "x".repeat(501);
    let err = store.replace_apply(work, &long, "y", &json!({}), &[], "Ida").await.unwrap_err();
    assert!(err.to_string().contains("500"), "Suchtext höchstens 500 Zeichen: {err}");
    let err = store.replace_apply(work, "", "y", &json!({}), &[], "Ida").await.unwrap_err();
    assert!(matches!(err, StoreError::Replace(_)), "leerer Suchtext abgelehnt: {err}");
    let err = store.replace_apply(work, "a", &"y".repeat(2001), &json!({}), &[], "Ida").await.unwrap_err();
    assert!(err.to_string().contains("2000"), "Ersatz höchstens 2000 Zeichen: {err}");
    let edits: Vec<ReplaceEdit> = (0..501).map(|k| ReplaceEdit { key: k, base_rev: 1, body: "x" }).collect();
    let err = store.replace_apply(work, "a", "b", &json!({}), &edits, "Ida").await.unwrap_err();
    assert!(err.to_string().contains("500 Abschnitte"), "höchstens 500 Abschnitte: {err}");
    let twice = [ReplaceEdit { key: 1, base_rev: 1, body: "x" }, ReplaceEdit { key: 1, base_rev: 1, body: "y" }];
    let err = store.replace_apply(work, "a", "b", &json!({}), &twice, "Ida").await.unwrap_err();
    assert!(err.to_string().contains("zweimal"), "ein Abschnitt höchstens einmal: {err}");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM replace_batch").fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 0, "abgelehnte Aufrufe schreiben kein Protokoll");
}
