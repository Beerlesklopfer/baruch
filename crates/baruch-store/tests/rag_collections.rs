// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sammlungen je Werk und Remote-Weg im Store (Migration 0031, Runde 12, Aufgabe „pgvector“): leer ohne
//! Zeile, Reihenfolge der Wahl mit lückenloser `position`, Doppelte entfernt, Ablehnungen ohne Änderung
//! (LanceDB, leer, mehrzeilig, zu lang, mehr als 100), Wege getrennt, unbekanntes Werk, Wachen der Tabelle.

use baruch_store::{RagWay, Store, StoreError};

async fn open() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.work_list().await.unwrap()[0].id;
    (dir, store, work)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// (Sammlung, position) eines Werks und Wegs, wie sie in der Tabelle stehen.
async fn rows(store: &Store, work: i64, way: &str) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT collection, position FROM work_rag_collection WHERE work_id = ? AND way = ? ORDER BY position",
    )
    .bind(work)
    .bind(way)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn without_rows_no_collection_is_chosen() {
    let (_dir, store, work) = open().await;
    for way in [RagWay::Pgvector, RagWay::Multigpt] {
        assert!(
            store.work_collections(work, way).await.unwrap().is_empty(),
            "{way}: ohne Zeile keine Sammlung (Weg aus)"
        );
    }
}

#[tokio::test]
async fn the_choice_keeps_its_order_trims_and_drops_doubles() {
    let (_dir, store, work) = open().await;
    let saved = store
        .set_work_collections(work, RagWay::Pgvector, &names(&[" Senfkorn ", "Leuchtturm", "Senfkorn", "Ährenfeld"]))
        .await
        .unwrap();
    assert_eq!(saved, names(&["Senfkorn", "Leuchtturm", "Ährenfeld"]), "beschnitten, der erste von Doppelten gewinnt");
    assert_eq!(
        store.work_collections(work, RagWay::Pgvector).await.unwrap(),
        saved,
        "Lesen liefert die gespeicherte Reihenfolge"
    );
    assert_eq!(
        rows(&store, work, "pgvector").await,
        vec![("Senfkorn".into(), 1), ("Leuchtturm".into(), 2), ("Ährenfeld".into(), 3)],
        "position 1..n lückenlos in der Reihenfolge der Wahl"
    );
    store.set_work_collections(work, RagWay::Pgvector, &names(&["Ährenfeld", "Senfkorn"])).await.unwrap();
    assert_eq!(
        rows(&store, work, "pgvector").await,
        vec![("Ährenfeld".into(), 1), ("Senfkorn".into(), 2)],
        "nach dem Ersetzen wieder lückenlos ab 1 (keine alte Zeile bleibt stehen)"
    );
    assert!(store.work_collections(work, RagWay::Multigpt).await.unwrap().is_empty(), "MultiGPT bleibt unberührt");
    store.set_work_collections(work, RagWay::Multigpt, &names(&["Leuchtturm"])).await.unwrap();
    assert_eq!(
        store.work_collections(work, RagWay::Pgvector).await.unwrap(),
        names(&["Ährenfeld", "Senfkorn"]),
        "die Wege sind getrennt: eine Wahl für MultiGPT ändert pgvector nicht"
    );
    store.set_work_collections(work, RagWay::Pgvector, &[]).await.unwrap();
    assert!(store.work_collections(work, RagWay::Pgvector).await.unwrap().is_empty(), "leere Liste = keine Sammlung");
    assert_eq!(rows(&store, work, "pgvector").await, vec![], "keine Zeilen mehr für pgvector");
}

#[tokio::test]
async fn invalid_choices_are_refused_and_change_nothing() {
    let (_dir, store, work) = open().await;
    store.set_work_collections(work, RagWay::Multigpt, &names(&["Senfkorn"])).await.unwrap();
    let long = "x".repeat(201);
    let many: Vec<String> = (0..101).map(|i| format!("Sammlung {i}")).collect();
    for (list, why) in [
        (names(&["  "]), "leer nach dem Beschneiden"),
        (names(&["a\nb"]), "mehrzeilig (Zeilenumbruch)"),
        (names(&["a\rb"]), "mehrzeilig (Wagenrücklauf)"),
        (vec![long], "201 Zeichen"),
        (many, "101 Sammlungen"),
    ] {
        let result = store.set_work_collections(work, RagWay::Multigpt, &list).await;
        assert!(matches!(result, Err(StoreError::Refused(_))), "{why} wird abgelehnt: {result:?}");
        assert_eq!(
            store.work_collections(work, RagWay::Multigpt).await.unwrap(),
            names(&["Senfkorn"]),
            "{why}: nichts geändert"
        );
    }
    let exactly: Vec<String> = (0..100).map(|i| format!("Sammlung {i}")).collect();
    assert_eq!(
        store.set_work_collections(work, RagWay::Multigpt, &exactly).await.unwrap().len(),
        100,
        "100 Sammlungen gehen noch"
    );
    let edge = "ü".repeat(200);
    assert!(
        store.set_work_collections(work, RagWay::Multigpt, std::slice::from_ref(&edge)).await.is_ok(),
        "200 Zeichen (nicht Bytes) gehen"
    );
    let lance = store.set_work_collections(work, RagWay::Lancedb, &names(&["x"])).await;
    assert!(matches!(lance, Err(StoreError::Refused(_))), "LanceDB hat keine Sammlungen: {lance:?}");
    assert!(
        matches!(store.work_collections(work, RagWay::Lancedb).await, Err(StoreError::Refused(_))),
        "auch Lesen für LanceDB wird abgelehnt"
    );
    let unknown = store.set_work_collections(work + 999, RagWay::Pgvector, &names(&["x"])).await;
    assert!(matches!(unknown, Err(StoreError::UnknownWork(_))), "unbekanntes Werk: {unknown:?}");
    assert!(
        matches!(store.work_collections(work + 999, RagWay::Pgvector).await, Err(StoreError::UnknownWork(_))),
        "Lesen für ein unbekanntes Werk"
    );
}

#[tokio::test]
async fn the_table_guards_itself_too() {
    let (_dir, store, work) = open().await;
    for (way, collection, position, why) in [
        ("lancedb", "x", 1, "kein Weg lancedb"),
        ("pgvector", "", 1, "leerer Name"),
        ("pgvector", "a\nb", 1, "Zeilenumbruch"),
        ("pgvector", "x", 0, "position ab 1"),
    ] {
        let result =
            sqlx::query("INSERT INTO work_rag_collection (work_id, way, collection, position) VALUES (?, ?, ?, ?)")
                .bind(work)
                .bind(way)
                .bind(collection)
                .bind(position)
                .execute(store.pool())
                .await;
        assert!(result.is_err(), "CHECK lehnt ab: {why}");
    }
}
