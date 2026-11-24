// SPDX-License-Identifier: AGPL-3.0-or-later
//! Statusübergänge der Revisionen (Migration 0004, Trigger `revision_status_transition`, ersetzt den
//! aus 0003) auf einer echten SQLite-Datei im Temp-Ordner. Erlaubt sind nur: jede nicht aktuelle
//! Fassung (proposed, rejected, superseded) → current (Umschalten), current → superseded und
//! proposed → rejected (Verwerfen). Alles andere, auch derselbe Status, lehnt die Datenbank ab.

use baruch_core::RevisionStatus;
use baruch_store::{Store, StoreError};
use sqlx::SqlitePool;

const REFUSED: &str = "Unzulässiger Statuswechsel";

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

/// Id der Revision `rev` im Abschnitt `seq`.
async fn revision_id(pool: &SqlitePool, seq: i64, rev: i64) -> i64 {
    sqlx::query_scalar("SELECT r.id FROM revision r JOIN node n ON n.id = r.node_id WHERE n.seq = ? AND r.number = ?")
        .bind(seq)
        .bind(rev)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("Revision {rev} in Abschnitt {seq} fehlt: {e}"))
}

/// Irgendeine Revision mit diesem Status (Id und Abschnitts-Id).
async fn any_with(pool: &SqlitePool, status: &str) -> (i64, i64) {
    sqlx::query_as("SELECT id, node_id FROM revision WHERE status = ? ORDER BY id LIMIT 1")
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("keine Revision mit Status {status}: {e}"))
}

async fn status(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM revision WHERE id = ?").bind(id).fetch_one(pool).await.unwrap()
}

async fn set(pool: &SqlitePool, id: i64, to: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE revision SET status = ? WHERE id = ?").bind(to).bind(id).execute(pool).await.map(|_| ())
}

/// Prüft, dass der Wechsel abgelehnt wird und der Status bleibt.
async fn assert_refused(pool: &SqlitePool, id: i64, to: &str) {
    let before = status(pool, id).await;
    let err = set(pool, id, to).await.expect_err(&format!("{before} → {to} hätte abgelehnt werden müssen"));
    assert!(err.to_string().contains(REFUSED), "{before} → {to}: falscher Grund der Ablehnung: {err}");
    assert_eq!(status(pool, id).await, before, "{before} → {to}: Status hat sich trotz Ablehnung geändert");
}

#[tokio::test]
async fn a_proposal_may_become_current_after_the_old_current_is_superseded() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    let proposal = revision_id(pool, 2, 3).await;
    assert_eq!(status(pool, proposal).await, "proposed", "Beispielwerk: Abschnitt 2 hat Vorschlag 3");
    let node: i64 =
        sqlx::query_scalar("SELECT node_id FROM revision WHERE id = ?").bind(proposal).fetch_one(pool).await.unwrap();
    let (current,): (i64,) = sqlx::query_as("SELECT id FROM revision WHERE node_id = ? AND status = 'current'")
        .bind(node)
        .fetch_one(pool)
        .await
        .unwrap();
    set(pool, current, "superseded").await.expect("current → superseded ist erlaubt");
    set(pool, proposal, "current").await.expect("proposed → current ist erlaubt");
    assert_eq!(status(pool, current).await, "superseded", "alte Fassung ist abgelöst");
    assert_eq!(status(pool, proposal).await, "current", "Vorschlag ist jetzt aktuell");
}

#[tokio::test]
async fn a_proposal_may_be_rejected() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    let (proposal, _) = any_with(pool, "proposed").await;
    set(pool, proposal, "rejected").await.expect("proposed → rejected ist erlaubt");
    assert_eq!(status(pool, proposal).await, "rejected", "Vorschlag ist verworfen");
}

#[tokio::test]
async fn the_current_revision_may_be_superseded() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    let (current, _) = any_with(pool, "current").await;
    set(pool, current, "superseded").await.expect("current → superseded ist erlaubt");
    assert_eq!(status(pool, current).await, "superseded", "aktuelle Fassung ist abgelöst");
}

/// Löst die aktuelle Fassung des Abschnitts ab, damit ein Wechsel auf current nicht erst am Teilindex
/// scheitert, sondern am Trigger geprüft wird.
async fn free_current(pool: &SqlitePool, node: i64) {
    let (current,): (i64,) = sqlx::query_as("SELECT id FROM revision WHERE node_id = ? AND status = 'current'")
        .bind(node)
        .fetch_one(pool)
        .await
        .unwrap();
    set(pool, current, "superseded").await.expect("current → superseded ist erlaubt");
}

async fn node_of(pool: &SqlitePool, id: i64) -> i64 {
    sqlx::query_scalar("SELECT node_id FROM revision WHERE id = ?").bind(id).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn a_superseded_version_may_become_current_again() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    let old = revision_id(pool, 2, 1).await;
    assert_eq!(status(pool, old).await, "superseded", "Beispielwerk: Abschnitt 2, Fassung 1 ist ersetzt");
    free_current(pool, node_of(pool, old).await).await;
    set(pool, old, "current").await.expect("superseded → current ist Umschalten und erlaubt");
    assert_eq!(status(pool, old).await, "current", "die alte Fassung trägt wieder die Markierung");
}

#[tokio::test]
async fn a_rejected_proposal_may_become_current() {
    let (_dir, store, work) = fresh().await;
    let pool = store.pool();
    store.reject(work, 10, 2).await.expect("Vorschlag 2 in Abschnitt 10 lässt sich verwerfen");
    let rejected = revision_id(pool, 10, 2).await;
    free_current(pool, node_of(pool, rejected).await).await;
    set(pool, rejected, "current").await.expect("rejected → current ist Umschalten und erlaubt");
    assert_eq!(status(pool, rejected).await, "current", "auch ein verworfener Vorschlag lässt sich verwenden");
}

#[tokio::test]
async fn the_current_version_cannot_be_rejected() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    let (current, _) = any_with(pool, "current").await;
    // Verwerfen ist ein Status für Vorschläge; die aktuelle Fassung wird nur durch Umschalten abgelöst.
    assert_refused(pool, current, "rejected").await;
}

#[tokio::test]
async fn a_rejected_proposal_cannot_become_a_proposal_again() {
    let (_dir, store, work) = fresh().await;
    let pool = store.pool();
    store.reject(work, 10, 2).await.unwrap();
    let rejected = revision_id(pool, 10, 2).await;
    // Zurück zu „offen“ gibt es nicht; wer ihn will, schaltet auf ihn um.
    assert_refused(pool, rejected, "proposed").await;
}

#[tokio::test]
async fn every_other_status_change_is_refused_by_the_database() {
    let (_dir, store, work) = fresh().await;
    let pool = store.pool();
    store.reject(work, 10, 2).await.expect("Vorschlag 2 in Abschnitt 10 lässt sich verwerfen");
    let rejected = revision_id(pool, 10, 2).await;
    let (superseded, _) = any_with(pool, "superseded").await;
    let (proposed, _) = any_with(pool, "proposed").await;
    let (current, _) = any_with(pool, "current").await;

    assert_refused(pool, superseded, "proposed").await;
    assert_refused(pool, superseded, "rejected").await;
    assert_refused(pool, rejected, "proposed").await;
    assert_refused(pool, rejected, "superseded").await;
    assert_refused(pool, current, "proposed").await;
    assert_refused(pool, current, "rejected").await;
    assert_refused(pool, proposed, "superseded").await;

    // Derselbe Status gilt nicht als Übergang.
    for (id, same) in [(proposed, "proposed"), (current, "current"), (superseded, "superseded"), (rejected, "rejected")]
    {
        assert_refused(pool, id, same).await;
    }
}

#[tokio::test]
async fn accept_reject_and_save_keep_working_with_the_trigger() {
    let (_dir, store, work) = fresh().await;
    let change = store.accept(work, 2, 3).await.expect("Übernehmen nutzt nur erlaubte Übergänge");
    assert_eq!(change.rev, 3, "Vorschlag 3 ist übernommen");
    assert_eq!(
        store.reject(work, 10, 2).await.expect("Verwerfen nutzt nur erlaubte Übergänge"),
        1,
        "die aktuelle Fassung bleibt 1"
    );
    let saved = store
        .save_human(work, 0, "= Vorwort\n\nMit Trigger.\n", "Autor")
        .await
        .expect("Speichern nutzt nur erlaubte Übergänge");
    assert!(saved.is_some(), "neuer Text ergibt eine neue Revision");

    let loaded = store.load_work(work).await.unwrap();
    for node in &loaded.nodes {
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
        assert_eq!(currents, 1, "Abschnitt {}: genau eine current", node.id);
    }
    assert_eq!(loaded.nodes[2].revisions[2].status, RevisionStatus::Current, "übernommener Vorschlag ist aktuell");
    assert_eq!(
        loaded.nodes[10].revisions[1].status,
        RevisionStatus::Rejected,
        "verworfener Vorschlag bleibt verworfen"
    );

    // Ein zweites Verwerfen derselben Revision bleibt eine fachliche Ablehnung des Stores.
    let again = store.reject(work, 10, 2).await;
    assert!(matches!(again, Err(StoreError::Refused(_))), "Store lehnt vor dem Trigger ab: {again:?}");
}
