// SPDX-License-Identifier: AGPL-3.0-or-later
//! Beiträge bearbeiten (Runde 28, Migration 0046): die lesenden Abfragen (`post_versions`, `thread_post_versions`,
//! `post_edited_map`, `PostInfo::edited`, `ThreadPost::edited`) und das Bearbeiten selbst (chat-auftrag): alte Fassung
//! als `post_version`, `edited` gesetzt, Grenzen, fremdes Werk, Faden. Echte SQLite-Datei im Temp-Ordner, Testtexte
//! frei erfunden.

use baruch_core::Post;
use baruch_store::{Store, StoreError, WorkId};

const MUEHLE: &str = "\
= Die Mühle

Das Rad dreht sich langsam.
";

async fn mill() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(MUEHLE, "muehle.typ", "Ole").await.unwrap().work;
    (dir, store, work)
}

#[tokio::test]
async fn a_fresh_post_has_no_versions_and_is_not_edited() {
    let (_dir, store, work) = mill().await;
    let post = Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "Wie schnell dreht das Rad?".into(),
        sources: vec![],
        refs: vec![],
    };
    let id = store.add_post(work, 0, &post).await.unwrap();
    assert_eq!(store.post_get(work, id).await.unwrap().edited, None, "nie bearbeitet: edited None");
    assert!(store.post_versions(work, id).await.unwrap().is_empty(), "keine alten Fassungen");
    assert!(store.post_edited_map(work).await.unwrap().is_empty(), "kein bearbeiteter Beitrag im Werk");
    let other = store.default_work().await.unwrap();
    let err = store.post_versions(other, id).await.unwrap_err();
    assert!(
        matches!(err, StoreError::UnknownPost { work: w, post } if w == other && post == id),
        "fremdes Werk: {err:?}"
    );

    let (summary, first) = store.thread_create(work, "Autor", "Wann kommt das Mehl?").await.unwrap();
    assert_eq!(first.edited, None, "neuer Fadenbeitrag: edited None");
    assert!(store.thread_post_versions(work, summary.id, first.id).await.unwrap().is_empty(), "keine alten Fassungen");
    assert!(store.thread_post_versions(work, summary.id, first.id + 100).await.is_err(), "unbekannter Fadenbeitrag");
}

fn human(body: &str) -> Post {
    Post { id: 0, created: String::new(), author: "Autor".into(), body: body.into(), sources: vec![], refs: vec![] }
}

#[tokio::test]
async fn editing_a_post_keeps_the_old_wording_as_a_version_and_marks_the_post_edited() {
    let (_dir, store, work) = mill().await;
    let id = store.add_post(work, 0, &human("Wie schnell dreht das Rad?")).await.unwrap();
    let created = store.post_get(work, id).await.unwrap().created;

    let (after, old) = store.post_edit(work, id, "  Wie langsam dreht das Rad?  ", "Autor").await.unwrap();
    assert_eq!(after.body, "Wie langsam dreht das Rad?", "neuer Text, getrimmt wie beim Anlegen");
    assert_eq!(old.body, "Wie schnell dreht das Rad?", "die alte Fassung bleibt wörtlich erhalten");
    assert_eq!(old.valid_from, created, "die erste Fassung galt seit dem Anlegen des Beitrags");
    assert_eq!(old.edited_by, "Autor", "wer ersetzt hat, steht dabei");
    assert_eq!(old.uuid.len(), 36, "die alte Fassung trägt eine uuid (Fülltrigger aus 0046)");
    assert_eq!(after.edited.as_deref(), Some(old.replaced.as_str()), "edited = Zeit der Änderung = replaced");
    assert_eq!(after.key, store.post_get(work, id).await.unwrap().key, "Abschnitt und Stelle bleiben");

    let (second, older) = store.post_edit(work, id, "Dreht das Rad überhaupt?", "Autor").await.unwrap();
    assert_eq!(older.valid_from, old.replaced, "die zweite Fassung galt seit der ersten Änderung");
    let versions = store.post_versions(work, id).await.unwrap();
    let bodies: Vec<&str> = versions.iter().map(|v| v.body.as_str()).collect();
    assert_eq!(bodies, ["Wie schnell dreht das Rad?", "Wie langsam dreht das Rad?"], "Verlauf, älteste zuerst");
    let map = store.post_edited_map(work).await.unwrap();
    assert_eq!(map.get(&id), second.edited.as_ref(), "der Beitrag steht mit der letzten Änderung in der Übersicht");
}

#[tokio::test]
async fn an_empty_unchanged_too_long_or_foreign_edit_changes_nothing() {
    let (_dir, store, work) = mill().await;
    let id = store.add_post(work, 0, &human("Das Mehl ist fein.")).await.unwrap();
    for (body, why) in [
        ("   ", "leer"),
        ("Das Mehl ist fein.", "unverändert"),
        (" Das Mehl ist fein. ", "unverändert bis auf Leerraum"),
    ] {
        let err = store.post_edit(work, id, body, "Autor").await.unwrap_err();
        assert!(matches!(err, StoreError::PostEdit(_)), "{why}: PostEdit statt stiller Änderung: {err:?}");
    }
    let long = "a".repeat(baruch_store::post_edit::MAX_POST_CHARS + 1);
    let err = store.post_edit(work, id, &long, "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::PostEdit(ref m) if m.contains("4000")), "zu lang: {err:?}");
    let other = store.default_work().await.unwrap();
    let err = store.post_edit(other, id, "Anders.", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownPost { .. }), "fremdes Werk: unbekannt: {err:?}");
    assert_eq!(store.post_get(work, id).await.unwrap().body, "Das Mehl ist fein.", "nichts geändert");
    assert!(store.post_versions(work, id).await.unwrap().is_empty(), "keine leere oder doppelte Fassung im Verlauf");
    assert_eq!(store.post_get(work, id).await.unwrap().edited, None, "nicht als bearbeitet markiert");
}

#[tokio::test]
async fn a_thread_post_is_edited_with_a_version_and_only_inside_its_thread() {
    let (_dir, store, work) = mill().await;
    let (summary, first) = store.thread_create(work, "Autor", "Wann kommt das Mehl?").await.unwrap();
    let (after, old) =
        store.thread_post_edit(work, summary.id, first.id, "Wann kommt das Korn?", "Autor").await.unwrap();
    assert_eq!(after.body, "Wann kommt das Korn?", "neuer Text");
    assert_eq!((after.thread, after.position), (first.thread, first.position), "Faden und Stelle bleiben");
    assert_eq!(old.body, "Wann kommt das Mehl?", "alte Fassung");
    assert_eq!(old.valid_from, first.created, "galt seit dem Anlegen");
    assert!(after.edited.is_some(), "als bearbeitet markiert");
    let read = store.thread_get(work, summary.id).await.unwrap();
    assert_eq!(read.entries[0].body, "Wann kommt das Korn?", "thread_get liest den neuen Text");
    assert_eq!(read.entries[0].edited, after.edited, "und edited");
    let versions = store.thread_post_versions(work, summary.id, first.id).await.unwrap();
    assert_eq!(versions.len(), 1, "eine alte Fassung");
    let (other, _) = store.thread_create(work, "Autor", "Zweiter Faden").await.unwrap();
    let err = store.thread_post_edit(work, other.id, first.id, "x", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownPost { .. }), "Beitrag eines anderen Fadens ist unbekannt: {err:?}");
    let err = store.thread_post_edit(work, 9999, first.id, "x", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::Refused(_)), "unbekannter Faden: {err:?}");
    assert!(
        store.post_edited_map(work).await.unwrap().is_empty(),
        "Fadenbeiträge zählen nicht zu den bearbeiteten Beiträgen des Abschnitts-Chats"
    );
}

#[tokio::test]
async fn deleting_an_edited_post_takes_its_versions_along() {
    let (_dir, store, work) = mill().await;
    let id = store.add_post(work, 0, &human("Erst so.")).await.unwrap();
    let (_, old) = store.post_edit(work, id, "Dann so.", "Autor").await.unwrap();
    store.post_delete(work, id).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM post_version").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "die Fassungen gehören zum Beitrag (Kaskade aus 0046)");
    let stone: i64 = sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE uuid = ? AND kind = 'post_version'")
        .bind(&old.uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(stone, 1, "ein Grabstein hält das Löschen der alten Fassung für den Austausch fest");
}
