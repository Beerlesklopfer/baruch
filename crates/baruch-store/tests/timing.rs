// SPDX-License-Identifier: AGPL-3.0-or-later
//! Timekeeper in the store (Round 16b, migration 0039): default without a row, setting and reading, limits, unknown
//! work, deleting the work. Works come through `import_typst` (freely invented text), so that the tests do not depend
//! on the fields of `NewWork`.

use baruch_store::{Store, StoreError, TimingSettings};

const SERMON: &str = "= Einleitung\n\nDer Weg ist weit.\n\n= Schluss\n\nGeht in Frieden.\n";

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(SERMON, "weg.typ", "Autor").await.unwrap().work;
    (dir, store, work)
}

async fn rows(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM work_timing").fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn without_a_row_the_default_of_110_words_and_no_target_applies() {
    let (_dir, store, work) = fresh().await;
    let settings = store.timing_settings(work).await.unwrap();
    assert_eq!(settings, TimingSettings::default(), "ohne Zeile gilt die Vorgabe");
    assert_eq!(
        settings,
        TimingSettings { words_per_minute: 110, target_seconds: None },
        "die Vorgabe des Vertrags: 110 Wörter je Minute, keine Zielzeit"
    );
    assert_eq!(rows(&store).await, 0, "Lesen legt keine Zeile an");
}

#[tokio::test]
async fn settings_are_stored_replaced_and_cleared() {
    let (_dir, store, work) = fresh().await;
    let first = TimingSettings { words_per_minute: 95, target_seconds: Some(20 * 60) };
    store.set_timing_settings(work, first).await.unwrap();
    assert_eq!(store.timing_settings(work).await.unwrap(), first, "gesetzt und wieder gelesen");
    let second = TimingSettings { words_per_minute: 130, target_seconds: None };
    store.set_timing_settings(work, second).await.unwrap();
    assert_eq!(store.timing_settings(work).await.unwrap(), second, "ersetzt, Zielzeit wieder leer");
    assert_eq!(rows(&store).await, 1, "eine Zeile je Werk, kein Anhängen");
}

#[tokio::test]
async fn the_limits_themselves_are_allowed() {
    let (_dir, store, work) = fresh().await;
    for settings in [
        TimingSettings { words_per_minute: 60, target_seconds: Some(60) },
        TimingSettings { words_per_minute: 250, target_seconds: Some(14_400) },
    ] {
        store.set_timing_settings(work, settings).await.expect("Grenzwerte sind erlaubt");
        assert_eq!(store.timing_settings(work).await.unwrap(), settings, "Grenzwert gespeichert: {settings:?}");
    }
}

#[tokio::test]
async fn values_outside_the_limits_are_refused_and_nothing_changes() {
    let (_dir, store, work) = fresh().await;
    let kept = TimingSettings { words_per_minute: 100, target_seconds: Some(900) };
    store.set_timing_settings(work, kept).await.unwrap();
    for wrong in [
        TimingSettings { words_per_minute: 59, target_seconds: None },
        TimingSettings { words_per_minute: 251, target_seconds: None },
        TimingSettings { words_per_minute: 110, target_seconds: Some(59) },
        TimingSettings { words_per_minute: 110, target_seconds: Some(14_401) },
        TimingSettings { words_per_minute: 110, target_seconds: Some(0) },
    ] {
        let result = store.set_timing_settings(work, wrong).await;
        assert!(
            matches!(&result, Err(StoreError::Refused(m)) if m.contains("erlaubt")),
            "außerhalb der Grenzen abgelehnt mit deutscher Meldung: {wrong:?} → {result:?}"
        );
        assert_eq!(store.timing_settings(work).await.unwrap(), kept, "nach der Ablehnung gilt der alte Stand");
    }
}

#[tokio::test]
async fn an_unknown_work_is_reported_for_reading_and_writing() {
    let (_dir, store, _work) = fresh().await;
    let read = store.timing_settings(4711).await;
    assert!(matches!(read, Err(StoreError::UnknownWork(4711))), "Lesen eines unbekannten Werks: {read:?}");
    let write = store.set_timing_settings(4711, TimingSettings::default()).await;
    assert!(matches!(write, Err(StoreError::UnknownWork(4711))), "Schreiben eines unbekannten Werks: {write:?}");
    assert_eq!(rows(&store).await, 0, "für ein unbekanntes Werk entsteht keine Zeile");
}

#[tokio::test]
async fn deleting_the_work_takes_its_timing_row_along() {
    let (_dir, store, work) = fresh().await;
    let title = store.work_meta(work).await.unwrap().title;
    store.set_timing_settings(work, TimingSettings { words_per_minute: 120, target_seconds: Some(600) }).await.unwrap();
    assert_eq!(rows(&store).await, 1, "vor dem Löschen eine Zeile");
    store.delete_work(work, &title).await.unwrap();
    assert_eq!(rows(&store).await, 0, "ON DELETE CASCADE: die Zeile geht mit dem Werk");
}
