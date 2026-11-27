// SPDX-License-Identifier: AGPL-3.0-or-later
//! Metadata of posts (Round 30, migration 0047; owner afterwards chat-bedienung). S1 checks the fully implemented
//! queries `add_post_meta`, `post_meta_set`, `post_meta`, `work_post_meta`, `thread_post_meta_set`, `thread_post_meta`.
//! Real SQLite file in the temp directory, test texts freely invented.

use baruch_core::Post;
use baruch_store::{PostMeta, PostRequest, Store, StoreError, WorkId};

const LEUCHTTURM: &str = "\
= Der Leuchtturm

Das Licht kreist über dem Wasser.

= Der Wachturm

Oben steht ein Fernrohr.
";

async fn tower() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(LEUCHTTURM, "leuchtturm.typ", "Ida").await.unwrap().work;
    (dir, store, work)
}

fn post(author: &str, body: &str) -> Post {
    Post { id: 0, created: String::new(), author: author.into(), body: body.into(), sources: vec![], refs: vec![] }
}

fn answer_meta(reply_to: i64) -> PostMeta {
    PostMeta {
        basis_rev: Some(1),
        basis_label: "geprüft: aktuelle Fassung 1".into(),
        reply_to: Some(reply_to),
        repeat_of: None,
        relay_from: Some("@lektor".into()),
        request: Some(PostRequest {
            persona: "@schrift".into(),
            mode: "chat".into(),
            scope: "section".into(),
            text: "Lektor, was meinst du?".into(),
            rev: Some(1),
            ..PostRequest::default()
        }),
    }
}

#[tokio::test]
async fn post_metadata_is_written_with_the_post_and_read_back() {
    let (_dir, store, work) = tower().await;
    let question = store.add_post(work, 0, &post("Autor", "Lektor, was meinst du?")).await.unwrap();
    let meta = answer_meta(question);
    let answer = store.add_post_meta(work, 0, &post("@schrift", "Das Licht trägt."), &meta).await.unwrap();

    let read = store.post_meta(work, &[question, answer]).await.unwrap();
    assert_eq!(read.get(&answer), Some(&meta), "alle Felder samt Auftrag (JSON) kommen unverändert zurück");
    assert_eq!(read.get(&question), Some(&PostMeta::default()), "ein Beitrag ohne Metadaten liest sich leer");
    assert!(PostMeta::default().is_empty() && !meta.is_empty(), "is_empty unterscheidet leer und gesetzt");
    let whole = store.work_post_meta(work).await.unwrap();
    assert_eq!(whole.keys().copied().collect::<Vec<_>>(), vec![answer], "work_post_meta nennt nur nicht leere");

    let repeat = PostMeta { repeat_of: Some(answer), ..PostMeta::default() };
    store.post_meta_set(work, question, &repeat).await.unwrap();
    assert_eq!(store.post_meta(work, &[question]).await.unwrap()[&question], repeat, "post_meta_set ersetzt alles");
}

#[tokio::test]
async fn links_only_point_into_the_same_work_and_deleting_the_target_clears_them() {
    let (_dir, store, work) = tower().await;
    let other = store.default_work().await.unwrap();
    assert_ne!(other, work, "das Beispielwerk ist ein anderes Werk");
    let foreign = store.add_post(other, 0, &post("Autor", "Fremd.")).await.unwrap();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM post").fetch_one(store.pool()).await.unwrap();
    let err = store.add_post_meta(work, 0, &post("@lektor", "Antwort."), &answer_meta(foreign)).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownPost { post, .. } if post == foreign), "fremder Verweis: {err:?}");
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM post").fetch_one(store.pool()).await.unwrap();
    assert_eq!(after, before, "eine Transaktion: ohne gültigen Verweis wird auch der Beitrag nicht angelegt");
    let bad = PostMeta { basis_label: "x".repeat(301), ..PostMeta::default() };
    assert!(
        matches!(store.add_post_meta(work, 0, &post("@lektor", "a"), &bad).await, Err(StoreError::PostEdit(_))),
        "Grenze der Migration als ehrliche Meldung"
    );

    let question = store.add_post(work, 0, &post("Autor", "Frage")).await.unwrap();
    let answer = store.add_post_meta(work, 0, &post("@lektor", "Antwort"), &answer_meta(question)).await.unwrap();
    store.post_delete(work, question).await.unwrap();
    let meta = &store.post_meta(work, &[answer]).await.unwrap()[&answer];
    assert_eq!(meta.reply_to, None, "ON DELETE SET NULL: die Antwort bleibt, der Verweis fällt weg");
    assert_eq!(meta.basis_rev, Some(1), "die übrigen Metadaten bleiben");
}

#[tokio::test]
async fn thread_post_metadata_stays_inside_its_thread() {
    let (_dir, store, work) = tower().await;
    let (thread, first) = store.thread_create(work, "Autor", "Welche Lampe?").await.unwrap();
    let reply = store.thread_add_post(work, thread.id, "@lektor", "Die alte.", &[]).await.unwrap();
    let first = first.id;
    let meta = PostMeta { reply_to: Some(first), basis_label: "geprüft: Faden".into(), ..PostMeta::default() };
    store.thread_post_meta_set(work, reply.id, &meta).await.unwrap();
    assert_eq!(store.thread_post_meta(work, &[reply.id]).await.unwrap()[&reply.id], meta, "gelesen wie geschrieben");

    let (_, other) = store.thread_create(work, "Autor", "Anderer Faden").await.unwrap();
    let cross = PostMeta { reply_to: Some(other.id), ..PostMeta::default() };
    let err = store.thread_post_meta_set(work, reply.id, &cross).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownPost { .. }), "Verweis in einen anderen Faden: {err:?}");
    let elsewhere = store.default_work().await.unwrap();
    assert!(store.thread_post_meta(elsewhere, &[reply.id]).await.unwrap().is_empty(), "fremdes Werk sieht nichts");
}

// ---- Round 30c (chat-bedienung) -------------------------------------------------------------------------------------

#[tokio::test]
async fn a_thread_post_is_added_with_its_metadata_in_one_transaction() {
    let (_dir, store, work) = tower().await;
    let (thread, first) = store.thread_create(work, "Autor", "Lektor, welche Lampe?").await.unwrap();
    let meta = PostMeta {
        reply_to: Some(first.id),
        request: Some(PostRequest {
            persona: "@lektor".into(),
            mode: "chat".into(),
            scope: "thread".into(),
            text: "Lektor, welche Lampe?".into(),
            rev: None,
            ..PostRequest::default()
        }),
        ..PostMeta::default()
    };
    let reply = store.thread_add_post_meta(work, thread.id, "@lektor", "Die alte.", &[], &meta).await.unwrap();
    assert_eq!(reply.position, 1, "angehängt wie thread_add_post");
    assert_eq!(store.thread_post_meta(work, &[reply.id]).await.unwrap()[&reply.id], meta, "Metadaten mitgeschrieben");

    let (_, other) = store.thread_create(work, "Autor", "Anderer Faden").await.unwrap();
    let before = store.thread_get(work, thread.id).await.unwrap().entries.len();
    let cross = PostMeta { reply_to: Some(other.id), ..PostMeta::default() };
    let err = store.thread_add_post_meta(work, thread.id, "@lektor", "Quer.", &[], &cross).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownPost { .. }), "Verweis in einen anderen Faden: {err:?}");
    assert_eq!(
        store.thread_get(work, thread.id).await.unwrap().entries.len(),
        before,
        "eine Transaktion: ohne gültigen Verweis auch kein Beitrag"
    );
    let err = store.thread_add_post_meta(work, 999_999, "@lektor", "x", &[], &PostMeta::default()).await.unwrap_err();
    assert!(matches!(err, StoreError::Refused(_)), "unbekannter Faden: {err:?}");
}

#[tokio::test]
async fn deleting_a_thread_post_clears_links_to_it_and_leaves_the_rest() {
    let (_dir, store, work) = tower().await;
    let (thread, first) = store.thread_create(work, "Autor", "Welche Lampe?").await.unwrap();
    let meta = PostMeta { reply_to: Some(first.id), relay_from: Some("@exeget".into()), ..PostMeta::default() };
    let reply = store.thread_add_post_meta(work, thread.id, "@lektor", "Die alte.", &[], &meta).await.unwrap();
    let deleted = store.thread_post_delete(work, thread.id, first.id).await.unwrap();
    assert_eq!(deleted.body, "Welche Lampe?", "der gelöschte Beitrag kommt zurück (für den Commit)");
    let read = store.thread_get(work, thread.id).await.unwrap();
    assert_eq!(read.entries.iter().map(|p| p.id).collect::<Vec<_>>(), vec![reply.id], "nur die Antwort bleibt");
    let after = &store.thread_post_meta(work, &[reply.id]).await.unwrap()[&reply.id];
    assert_eq!(after.reply_to, None, "ON DELETE SET NULL: der Verweis fällt weg");
    assert_eq!(after.relay_from.as_deref(), Some("@exeget"), "die übrigen Metadaten bleiben");
    let broken: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check").fetch_one(store.pool()).await.unwrap();
    assert_eq!(broken, 0, "Fremdschlüssel intakt");

    let again = store.thread_post_delete(work, thread.id, first.id).await.unwrap_err();
    assert!(matches!(again, StoreError::UnknownPost { .. }), "zweimal löschen: {again:?}");
    let elsewhere = store.default_work().await.unwrap();
    let foreign = store.thread_post_delete(elsewhere, thread.id, reply.id).await.unwrap_err();
    assert!(matches!(foreign, StoreError::Refused(_)), "fremdes Werk kennt den Faden nicht: {foreign:?}");
}
