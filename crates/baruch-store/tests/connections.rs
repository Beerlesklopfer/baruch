// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindungen, RAG-Wege und bevorzugte Quellen im Store (Migration 0013, Runde 9): Vorgaben ohne
//! Zeile, Speichern und Prüfergebnis, Wege je Werk, lückenlose Reihenfolge der Quellen nach Anlegen,
//! Verschieben und Löschen, Ablehnungen ohne Änderung, Wachen der Tabelle.

use baruch_store::{ConnectionKind, NewSource, RagWay, SourceEdit, Store, StoreError};
use serde_json::json;

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn source(title: &str, url: &str) -> NewSource {
    NewSource { title: title.into(), url: url.into(), note: String::new() }
}

/// Reihenfolge als (Titel, Stelle).
async fn order(store: &Store) -> Vec<(String, i64)> {
    store.sources_list().await.unwrap().into_iter().map(|s| (s.title, s.position)).collect()
}

#[tokio::test]
async fn without_rows_every_connection_shows_its_default() {
    let (_dir, store) = open().await;
    let list = store.connection_list().await.unwrap();
    let kinds: Vec<ConnectionKind> = list.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, ConnectionKind::ALL, "alle vier in fester Reihenfolge");
    for row in &list {
        assert_eq!(row.config, row.kind.default_config(), "{}: Vorgabe ohne Zeile", row.kind);
        assert!(row.updated.is_none() && row.check.is_none(), "{}: nie gespeichert, nie geprüft", row.kind);
    }
    assert_eq!(list[0].config, json!({ "url": "http://127.0.0.1:1234" }), "Eliezer nach Vertrag");
    assert_eq!(
        list[1].config,
        json!({ "url": "" }),
        "MultiGPT ohne Adresse (Vertrag R24 §4): jede Instanz trägt ihre eigene ein, keine fremde Vorgabe im Repo"
    );
    assert_eq!(
        list[2].config,
        json!({
            "host": "", "port": 5432, "database": "", "user": "", "tls": "prefer",
            "table": "chunks", "text_column": "content", "vector_column": "embedding",
            "collection_column": "collection", "document_column": "document", "section_column": "",
            "title_column": "", "url_column": "", "model_column": "", "embedding_model": "nomic-embed-text",
        }),
        "pgvector nach Vertrag (Runde 12, §5: Zuordnung von Tabelle und Spalten, Einbettungsmodell)"
    );
    assert_eq!(list[3].config, json!({ "dir": "lancedb", "embedding_model": "nomic-embed-text" }), "LanceDB");
}

#[tokio::test]
async fn a_saved_multigpt_address_stays_when_the_default_is_empty() {
    // Runde 24: Die Vorgabe für MultiGPT ist leer. Eine Instanz, die ihre Adresse gespeichert hat, behält sie –
    // ohne Migration, weil die Vorgabe nur gilt, solange keine Zeile da ist.
    let dir = tempfile::tempdir().unwrap();
    let saved = json!({ "url": "http://127.0.0.1:9/mcp/" });
    let updated = {
        let store = Store::open(dir.path()).await.unwrap();
        let row = store.connection_set(ConnectionKind::Multigpt, &saved).await.unwrap();
        assert_eq!(row.config, saved, "gespeichert, wie übergeben");
        store.pool().close().await;
        row.updated
    };
    let store = Store::open(dir.path()).await.unwrap();
    let row = store.connection_get(ConnectionKind::Multigpt).await.unwrap();
    assert_eq!(row.config, saved, "nach dem erneuten Öffnen dieselbe Adresse, nicht die leere Vorgabe");
    assert_eq!(row.updated, updated, "die Zeile wurde beim Öffnen nicht angefasst");
    assert_eq!(
        ConnectionKind::Multigpt.default_config(),
        json!({ "url": "" }),
        "die Vorgabe selbst nennt keine Adresse (Vertrag R24 §4)"
    );
}

#[tokio::test]
async fn saving_a_config_keeps_it_and_a_change_clears_the_old_check() {
    let (_dir, store) = open().await;
    let config = json!({ "url": "http://127.0.0.1:9" });
    let saved = store.connection_set(ConnectionKind::Eliezer, &config).await.unwrap();
    assert_eq!(saved.config, config, "gespeichert, wie übergeben");
    assert!(saved.updated.is_some(), "jetzt mit Zeitpunkt");

    let checked = store
        .connection_record_check(ConnectionKind::Eliezer, true, "Verbindung steht", &json!({ "models": ["m"] }))
        .await
        .unwrap();
    let check = checked.check.expect("Prüfung gespeichert");
    assert!(check.ok && check.message == "Verbindung steht", "Ergebnis wie übergeben: {check:?}");
    assert_eq!(check.details, json!({ "models": ["m"] }), "Angaben als JSON");
    assert!(check.at.ends_with('Z'), "Zeitpunkt in UTC: {}", check.at);

    let same = store.connection_set(ConnectionKind::Eliezer, &config).await.unwrap();
    assert!(same.check.is_some(), "dieselben Einstellungen lassen die Prüfung stehen");
    let other = store.connection_set(ConnectionKind::Eliezer, &json!({ "url": "http://127.0.0.1:10" })).await.unwrap();
    assert!(other.check.is_none(), "neue Adresse: die alte Prüfung galt ihr nicht");
    assert_eq!(
        store.connection_get(ConnectionKind::Multigpt).await.unwrap().config,
        ConnectionKind::Multigpt.default_config(),
        "andere Verbindungen bleiben bei der Vorgabe"
    );
}

#[tokio::test]
async fn the_secret_binding_survives_config_changes_and_is_cleared_explicitly() {
    let (_dir, store) = open().await;
    let kind = ConnectionKind::Eliezer;
    store.connection_bind_secret(kind, None).await.unwrap();
    assert!(store.connection_get(kind).await.unwrap().updated.is_none(), "None ohne Zeile legt keine an (Vorgabe)");
    store.connection_bind_secret(kind, Some("http://127.0.0.1:1234")).await.unwrap();
    let row = store.connection_get(kind).await.unwrap();
    assert_eq!(row.secret_target.as_deref(), Some("http://127.0.0.1:1234"), "Ziel gemerkt");
    assert_eq!(row.config, kind.default_config(), "ohne Zeile mit der Vorgabe angelegt");
    // Neue Einstellungen ändern die Bindung nicht: Sie hält fest, wohin das Geheimnis gehört.
    let moved = store.connection_set(kind, &json!({ "url": "http://127.0.0.1:9" })).await.unwrap();
    assert_eq!(moved.secret_target.as_deref(), Some("http://127.0.0.1:1234"), "Bindung bleibt beim Speichern");
    store.connection_bind_secret(kind, None).await.unwrap();
    assert_eq!(store.connection_get(kind).await.unwrap().secret_target, None, "None: gilt für das jetzige Ziel");
}

#[tokio::test]
async fn a_check_of_the_default_creates_the_row_with_the_default_config() {
    let (_dir, store) = open().await;
    let row = store
        .connection_record_check(ConnectionKind::Pgvector, false, "Verbindung abgelehnt", &json!({}))
        .await
        .unwrap();
    assert_eq!(row.config, ConnectionKind::Pgvector.default_config(), "Vorgabe wird mitgespeichert");
    assert!(row.check.is_some_and(|c| !c.ok), "Fehlschlag gespeichert");
}

#[tokio::test]
async fn a_config_must_be_an_object_and_the_table_guards_it_too() {
    let (_dir, store) = open().await;
    let refused = store.connection_set(ConnectionKind::Lancedb, &json!(["dir"])).await;
    assert!(matches!(refused, Err(StoreError::Refused(_))), "Liste statt Objekt: {refused:?}");
    let raw = sqlx::query("INSERT INTO connection (kind, config) VALUES ('ollama', '{}')").execute(store.pool()).await;
    assert!(raw.is_err(), "die Tabelle kennt nur die vier Arten");
    let raw = sqlx::query("INSERT INTO connection (kind, config, checked) VALUES ('eliezer', '{}', 'jetzt')")
        .execute(store.pool())
        .await;
    assert!(raw.is_err(), "Zeitpunkt der Prüfung nur zusammen mit dem Ergebnis");
}

#[tokio::test]
async fn rag_ways_default_to_lancedb_and_can_be_empty() {
    let (_dir, store) = open().await;
    let work = store.default_work().await.unwrap();
    assert_eq!(store.work_rag(work).await.unwrap(), (vec![RagWay::Lancedb], true), "Vorgabe ohne Zeile");
    let set = store.set_work_rag(work, &[RagWay::Multigpt, RagWay::Lancedb, RagWay::Multigpt]).await.unwrap();
    assert_eq!(set, [RagWay::Lancedb, RagWay::Multigpt], "feste Reihenfolge, ohne Doppel");
    assert_eq!(store.work_rag(work).await.unwrap(), (set.clone(), false), "gespeichert, keine Vorgabe mehr");
    assert_eq!(store.set_work_rag(work, &[]).await.unwrap(), [], "leer = nur Volltext");
    assert_eq!(store.work_rag(work).await.unwrap(), (vec![], false), "leer bleibt leer, nicht Vorgabe");
    assert!(matches!(store.work_rag(4711).await, Err(StoreError::UnknownWork(4711))), "unbekanntes Werk");
    assert!(
        matches!(store.set_work_rag(4711, &[RagWay::Lancedb]).await, Err(StoreError::UnknownWork(4711))),
        "unbekanntes Werk wird nicht angelegt"
    );
}

#[tokio::test]
async fn sources_keep_a_gapless_order_through_create_move_and_delete() {
    let (_dir, store) = open().await;
    for (title, url) in [
        ("Atlas", "https://atlas.example/"),
        ("Brevier", "https://brevier.example/"),
        ("Chronik", "http://chronik.example/"),
    ] {
        store.source_create(&source(title, url)).await.unwrap();
    }
    assert_eq!(
        order(&store).await,
        [("Atlas".into(), 1), ("Brevier".into(), 2), ("Chronik".into(), 3)],
        "neue Quellen hinten, Stellen 1..n"
    );
    let chronik = store.sources_list().await.unwrap()[2].id;
    let moved = store.source_move(chronik, 1).await.unwrap();
    let titles: Vec<&str> = moved.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Chronik", "Atlas", "Brevier"], "nach vorn verschoben, die anderen rücken nach");
    store.source_move(chronik, 99).await.unwrap();
    assert_eq!(
        order(&store).await,
        [("Atlas".into(), 1), ("Brevier".into(), 2), ("Chronik".into(), 3)],
        "Stelle über n landet am Ende"
    );
    let atlas = store.sources_list().await.unwrap()[0].id;
    let gone = store.source_delete(atlas).await.unwrap();
    assert_eq!(gone.title, "Atlas", "die gelöschte Quelle kommt zurück");
    assert_eq!(order(&store).await, [("Brevier".into(), 1), ("Chronik".into(), 2)], "lückenlos nach dem Löschen");
    assert!(store.source_delete(atlas).await.is_err(), "zweimal löschen geht nicht");
    assert!(store.source_move(atlas, 1).await.is_err(), "eine gelöschte Quelle lässt sich nicht verschieben");
}

#[tokio::test]
async fn updating_a_source_changes_only_the_given_fields() {
    let (_dir, store) = open().await;
    let created = store
        .source_create(&NewSource {
            title: "  Lexikon ".into(),
            url: "https://lexikon.example/".into(),
            note: "gut".into(),
        })
        .await
        .unwrap();
    assert_eq!(created.title, "Lexikon", "Titel beschnitten");
    let edit = SourceEdit { note: Some("sehr gut".into()), ..SourceEdit::default() };
    let updated = store.source_update(created.id, &edit).await.unwrap();
    assert_eq!(
        (updated.title.as_str(), updated.url.as_str(), updated.note.as_str()),
        ("Lexikon", "https://lexikon.example/", "sehr gut"),
        "nur die Notiz geändert"
    );
    let bad = SourceEdit { url: Some("ftp://lexikon.example/".into()), ..SourceEdit::default() };
    assert!(matches!(store.source_update(created.id, &bad).await, Err(StoreError::Refused(_))), "kein http(s)");
    assert_eq!(store.sources_list().await.unwrap()[0].url, "https://lexikon.example/", "abgelehnt: nichts geändert");
    assert!(store.source_create(&source("", "https://x.example/")).await.is_err(), "leerer Titel abgelehnt");
    assert!(store.source_create(&source("X", "javascript:alert(1)")).await.is_err(), "nur http(s)");
    assert_eq!(store.sources_list().await.unwrap().len(), 1, "abgelehnte Quellen sind nicht angelegt");
    let raw = sqlx::query("INSERT INTO preferred_source (title, url, position) VALUES ('X', 'file:///etc', 9)")
        .execute(store.pool())
        .await;
    assert!(raw.is_err(), "auch die Tabelle lässt nur http(s) zu");
}
