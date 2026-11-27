// SPDX-License-Identifier: AGPL-3.0-or-later
//! Revisionsbaum (Vertrag Runde 3, Abschnitte 2 und 3) auf echten SQLite-Dateien im Temp-Ordner:
//! Migration 0004 (frisch und auf dem Stand 0001–0003), Regeln in der Datenbank selbst, Umschalten,
//! Speichern auf einer Basis, Ändern und Löschen durch den Menschen, Baum aller Fassungen.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use baruch_core::{Author, RevisionStatus, sample};
use baruch_store::{ChangeKind, RevisionEdit, RevisionRecord, Store, StoreError};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

/// `(number, based_on, status)` aller Revisionen des Abschnitts `seq`, nach Nummer.
async fn rows(pool: &SqlitePool, seq: i64) -> Vec<(i64, Option<i64>, String)> {
    sqlx::query_as(
        "SELECT r.number, r.based_on, r.status FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.seq = ? AND n.work_id = (SELECT min(id) FROM work) ORDER BY r.number",
    )
    .bind(seq)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn last_rev(pool: &SqlitePool, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT last_rev FROM node WHERE seq = ? AND work_id = (SELECT min(id) FROM work)")
        .bind(seq)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn node_db_id(pool: &SqlitePool, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE seq = ? AND work_id = (SELECT min(id) FROM work)")
        .bind(seq)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn statuses(work: &baruch_core::Work, node: usize) -> Vec<(u32, RevisionStatus)> {
    work.nodes[node].revisions.iter().map(|r| (r.number, r.status)).collect()
}

fn bodies(work: &baruch_core::Work, node: usize) -> Vec<String> {
    work.nodes[node].revisions.iter().map(|r| r.body.clone()).collect()
}

// ---- Migration 0004 -----------------------------------------------------------------------------

#[tokio::test]
async fn migration_0004_on_a_fresh_database_sets_parents_and_last_rev() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    assert_eq!(
        rows(pool, 2).await,
        vec![(1, None, "superseded".into()), (2, Some(1), "current".into()), (3, Some(2), "proposed".into())],
        "Beispielwerk: Einleitung als Kette 1 ← 2 ← 3"
    );
    assert_eq!(
        rows(pool, 10).await,
        vec![(1, None, "current".into()), (2, Some(1), "proposed".into())],
        "Vorschlag hängt an der aktuellen Fassung"
    );
    for seq in 0..11 {
        let max = rows(pool, seq).await.iter().map(|r| r.0).max().unwrap_or(0);
        assert_eq!(last_rev(pool, seq).await, max, "Abschnitt {seq}: last_rev = höchste Nummer");
    }
    let edited: i64 =
        sqlx::query_scalar("SELECT count(*) FROM revision WHERE edited IS NOT NULL").fetch_one(pool).await.unwrap();
    assert_eq!(edited, 0, "frisch ist keine Fassung vom Menschen geändert");
}

/// Datenbank nur mit den Migrationen 0001–0003 (Stand vor dem Revisionsbaum).
async fn database_at_0003(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version <= 3).cloned().collect();
    assert_eq!(old.len(), 3, "0001–0003 vorhanden");
    let migrator = sqlx::migrate::Migrator { migrations: Cow::Owned(old), ..sqlx::migrate::Migrator::DEFAULT };
    migrator.run(&pool).await.unwrap();
    pool
}

/// Das Beispielwerk so, wie der alte Store es schrieb: ohne `based_on`.
async fn insert_sample_without_parents(pool: &SqlitePool) {
    let work = sample::work();
    let work_id: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author) VALUES ('beispiel', 'book', ?, ?) RETURNING id",
    )
    .bind(&work.title)
    .bind(&work.author)
    .fetch_one(pool)
    .await
    .unwrap();
    for node in &work.nodes {
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, ?, ?, ?, ?, 'chapter') RETURNING id",
        )
        .bind(work_id)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(node.id as i64)
        .bind(&node.number)
        .bind(&node.title)
        .fetch_one(pool)
        .await
        .unwrap();
        for r in &node.revisions {
            let (kind, name, model) = match &r.author {
                Author::Human { name } => ("human", name.as_str(), None),
                Author::Assistant { persona, model } => ("assistant", persona.as_str(), Some(model.as_str())),
            };
            let status = match r.status {
                RevisionStatus::Proposed => "proposed",
                RevisionStatus::Current => "current",
                RevisionStatus::Rejected => "rejected",
                RevisionStatus::Superseded => "superseded",
            };
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(node_id)
            .bind(r.number as i64)
            .bind(&r.body)
            .bind(kind)
            .bind(name)
            .bind(model)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
        }
    }
}

#[tokio::test]
async fn migration_0004_on_a_0003_database_fills_parents_by_rule_r() {
    let dir = tempfile::tempdir().unwrap();
    let pool = database_at_0003(dir.path()).await;
    insert_sample_without_parents(&pool).await;
    // Ein Abschnitt mit verworfenem Vorschlag zwischen zwei eigenen Fassungen und einem Vorschlag,
    // dessen Basis der alte Store schon gesetzt hatte.
    let node_id: i64 = sqlx::query_scalar(
        "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
         VALUES (1, 11, NULL, 11, '5', 'Nachtrag', 'chapter') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, based_on) VALUES
           ({node_id}, 1, 'a', 'human', 'Autor', NULL, 'superseded', NULL),
           ({node_id}, 2, 'b', 'assistant', '@lektor', 'm', 'rejected', NULL),
           ({node_id}, 3, 'c', 'human', 'Autor', NULL, 'current', NULL),
           ({node_id}, 4, 'd', 'assistant', '@lektor', 'm', 'proposed', 1);"
    )))
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    // Store::open führt nur noch 0004 aus (0001–0003 stehen mit gleicher Prüfsumme in der Tabelle,
    // sonst schlüge open mit VersionMismatch fehl) und befüllt nichts neu.
    let store = Store::open(dir.path()).await.expect("0004 läuft auf dem Stand 0001–0003");
    let pool = store.pool();
    assert_eq!(store.works().await.unwrap().len(), 1, "keine neue Saat in eine Datenbank mit Werk");
    assert_eq!(
        rows(pool, 2).await,
        vec![(1, None, "superseded".into()), (2, Some(1), "current".into()), (3, Some(2), "proposed".into())],
        "Regel R: Elternteil = höchste frühere current/superseded"
    );
    assert_eq!(
        rows(pool, 11).await,
        vec![
            (1, None, "superseded".into()),
            (2, Some(1), "rejected".into()),
            (3, Some(1), "current".into()),
            (4, Some(1), "proposed".into())
        ],
        "verworfene Vorschläge sind kein Elternteil; eine vorhandene Basis bleibt"
    );
    assert_eq!(last_rev(pool, 11).await, 4, "last_rev aus der höchsten Nummer");
    assert_eq!(last_rev(pool, 0).await, 1, "last_rev für jeden Abschnitt gesetzt");
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(pool).await.unwrap();
    assert!(
        applied.starts_with(&[1, 2, 3, 4]),
        "0001–0004 sind eingetragen (spätere Migrationen dürfen folgen): {applied:?}"
    );
    // Und der Store arbeitet danach mit den neuen Regeln.
    assert_eq!(store.load_work(1).await.unwrap().nodes[11].revisions.len(), 4, "alle Fassungen bleiben");
}

// ---- Regeln in der Datenbank ----------------------------------------------------------------------

async fn refused(pool: &SqlitePool, sql: &str, reason: &str) {
    let err = sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .expect_err(&format!("hätte abgelehnt werden müssen: {sql}"));
    assert!(err.to_string().contains(reason), "{sql}: falscher Grund der Ablehnung: {err}");
}

const ORIGIN: &str = "Herkunft einer Revision ist unveränderlich";

#[tokio::test]
async fn the_database_refuses_to_delete_the_current_version() {
    let (_dir, store, work) = fresh().await;
    let before = store.load_work(work).await.unwrap();
    refused(
        store.pool(),
        "DELETE FROM revision WHERE status = 'current'",
        "Die aktuelle Fassung lässt sich nicht löschen",
    )
    .await;
    let after = store.load_work(work).await.unwrap();
    assert_eq!(serde_json::to_value(&after).unwrap(), serde_json::to_value(&before).unwrap(), "nichts gelöscht");
}

#[tokio::test]
async fn the_database_refuses_to_change_the_author_name() {
    let (_dir, store, _work) = fresh().await;
    refused(store.pool(), "UPDATE revision SET author_name = 'Fremd'", ORIGIN).await;
}

#[tokio::test]
async fn the_database_refuses_to_change_the_number() {
    let (_dir, store, _work) = fresh().await;
    refused(store.pool(), "UPDATE revision SET number = number + 100", ORIGIN).await;
}

#[tokio::test]
async fn the_database_refuses_to_change_the_model() {
    let (_dir, store, _work) = fresh().await;
    refused(store.pool(), "UPDATE revision SET model = 'anderes' WHERE model IS NOT NULL", ORIGIN).await;
}

#[tokio::test]
async fn the_database_refuses_to_change_the_creation_time() {
    let (_dir, store, _work) = fresh().await;
    refused(store.pool(), "UPDATE revision SET created = '2000-01-01T00:00:00Z'", ORIGIN).await;
}

#[tokio::test]
async fn the_database_refuses_to_move_a_revision_to_another_section() {
    let (_dir, store, _work) = fresh().await;
    let target = node_db_id(store.pool(), 0).await;
    refused(store.pool(), &format!("UPDATE revision SET node_id = {target} WHERE status = 'proposed'"), ORIGIN).await;
}

#[tokio::test]
async fn the_database_refuses_an_illegal_status_change() {
    let (_dir, store, _work) = fresh().await;
    // current → rejected: Verwerfen gibt es nur für Vorschläge.
    refused(
        store.pool(),
        "UPDATE revision SET status = 'rejected' WHERE status = 'current'",
        "Unzulässiger Statuswechsel",
    )
    .await;
}

#[tokio::test]
async fn the_database_refuses_a_parent_that_does_not_exist() {
    let (_dir, store, _work) = fresh().await;
    let node = node_db_id(store.pool(), 2).await;
    refused(
        store.pool(),
        &format!(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, based_on) \
             VALUES ({node}, 4, 'x', 'human', 'Autor', 'superseded', 77)"
        ),
        "Elternteil der Revision fehlt",
    )
    .await;
    refused(
        store.pool(),
        &format!("UPDATE revision SET based_on = 77 WHERE node_id = {node} AND number = 3"),
        "Elternteil der Revision fehlt",
    )
    .await;
}

#[tokio::test]
async fn the_database_refuses_a_parent_that_is_not_smaller() {
    let (_dir, store, _work) = fresh().await;
    let node = node_db_id(store.pool(), 2).await;
    // Fassung 2 auf Fassung 3 (größer) oder auf sich selbst stützen: kein Baum mehr.
    refused(
        store.pool(),
        &format!("UPDATE revision SET based_on = 3 WHERE node_id = {node} AND number = 2"),
        "Elternteil der Revision fehlt",
    )
    .await;
    refused(
        store.pool(),
        &format!("UPDATE revision SET based_on = 2 WHERE node_id = {node} AND number = 2"),
        "Elternteil der Revision fehlt",
    )
    .await;
}

#[tokio::test]
async fn the_database_refuses_to_reuse_a_number_after_deleting_the_highest() {
    let (_dir, store, work) = fresh().await;
    store.delete_revision(work, 2, 3, "Autor").await.expect("Vorschlag 3 lässt sich löschen");
    let node = node_db_id(store.pool(), 2).await;
    refused(
        store.pool(),
        &format!(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, based_on) \
             VALUES ({node}, 3, 'x', 'human', 'Autor', 'superseded', 2)"
        ),
        "Revisionsnummern werden nicht wiederverwendet",
    )
    .await;
    let next = store.add_proposal(work, 2, "= Einleitung\n\nneu\n", "@lektor", "m", "").await.unwrap();
    assert_eq!(next, 4, "der Store nimmt last_rev + 1, nicht max + 1");
}

// ---- Umschalten -----------------------------------------------------------------------------------

#[tokio::test]
async fn set_current_switches_to_a_superseded_version_without_changing_text() {
    let (_dir, store, work) = fresh().await;
    let before = store.load_work(work).await.unwrap();
    let change = store.set_current(work, 2, 1).await.unwrap();
    assert_eq!((change.rev, change.kind), (1, ChangeKind::Switched), "Umschalten auf eine ersetzte Fassung");
    assert_eq!(change.author, Author::Human { name: "Autor".into() }, "Autor der jetzt aktuellen Fassung");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 2),
        vec![(1, RevisionStatus::Current), (2, RevisionStatus::Superseded), (3, RevisionStatus::Proposed)],
        "genau eine current, die alte ist superseded, der Vorschlag bleibt offen"
    );
    assert_eq!(bodies(&after, 2), bodies(&before, 2), "kein Text verändert");
}

#[tokio::test]
async fn set_current_on_a_proposal_counts_as_accepted() {
    let (_dir, store, work) = fresh().await;
    let change = store.set_current(work, 2, 3).await.unwrap();
    assert_eq!(change.kind, ChangeKind::Accepted, "ein Vorschlag wird durch Umschalten übernommen");
    assert!(matches!(change.author, Author::Assistant { .. }), "Autor ist die Persona: {:?}", change.author);
    let after = store.load_work(work).await.unwrap();
    assert_eq!(after.nodes[2].current_number(), Some(3), "der Vorschlag 3 ist jetzt die aktuelle Fassung");
}

#[tokio::test]
async fn set_current_on_a_rejected_proposal_switches() {
    let (_dir, store, work) = fresh().await;
    store.reject(work, 10, 2).await.unwrap();
    let change = store.set_current(work, 10, 2).await.unwrap();
    assert_eq!(change.kind, ChangeKind::Switched, "ein verworfener Vorschlag ist eine andere Fassung");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 10),
        vec![(1, RevisionStatus::Superseded), (2, RevisionStatus::Current)],
        "verworfen → aktuell, die alte aktuelle ist ersetzt"
    );
}

#[tokio::test]
async fn set_current_on_the_current_version_is_refused() {
    let (_dir, store, work) = fresh().await;
    let result = store.set_current(work, 2, 2).await;
    assert!(
        matches!(result, Err(StoreError::Refused(m)) if m == "Diese Fassung ist schon aktuell."),
        "schon aktuell: {result:?}"
    );
    let err = store.set_current(work, 2, 9).await.unwrap_err();
    assert!(
        matches!(&err, StoreError::UnknownRevision { rev: 9, existing, .. } if existing == &[1, 2, 3]),
        "Fassung 9 gibt es nicht, die Meldung nennt 1–3: {err:?}"
    );
}

#[tokio::test]
async fn an_unknown_number_after_a_deletion_names_the_existing_numbers() {
    let (_dir, store, work) = fresh().await;
    // Einleitung: 1, 2 (current), 3 (Vorschlag); Fassung 4 speichern, dann 3 löschen → 1, 2, 4.
    store.save_human(work, 2, "= Einleitung\n\nVierte Fassung.\n", "Autor").await.unwrap().unwrap();
    store.delete_revision(work, 2, 3, "Autor").await.unwrap();
    let err = store.set_current(work, 2, 3).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Fassung 3 gibt es nicht; vorhanden: 1, 2, 4",
        "die Lücke nach dem Löschen wird nicht als Bereich 1–4 verschleiert"
    );
    let err = store.delete_revision(work, 2, 3, "Autor").await.unwrap_err();
    assert!(
        matches!(&err, StoreError::UnknownRevision { node: 2, rev: 3, existing } if existing == &[1, 2, 4]),
        "auch Löschen meldet die vorhandenen Nummern: {err:?}"
    );
}

// ---- Speichern auf einer Basis ---------------------------------------------------------------------

#[tokio::test]
async fn save_human_on_takes_the_shown_version_as_parent() {
    let (_dir, store, work) = fresh().await;
    let change = store.save_human_on(work, 2, "= Einleitung\n\nAuf Fassung 1.\n", "Autor", 1).await.unwrap().unwrap();
    assert_eq!((change.rev, change.kind), (4, ChangeKind::Saved), "neue Nummer hinter dem Vorschlag");
    assert_eq!(
        rows(store.pool(), 2).await,
        vec![
            (1, None, "superseded".into()),
            (2, Some(1), "superseded".into()),
            (3, Some(2), "proposed".into()),
            (4, Some(1), "current".into())
        ],
        "Elternteil ist die gezeigte Fassung 1, nicht die bisher aktuelle"
    );
    // Gleicher Text wie die Basis → nichts.
    let base = store.load_work(work).await.unwrap().nodes[2].revision(1).unwrap().body.clone();
    assert_eq!(store.save_human_on(work, 2, &base, "Autor", 1).await.unwrap(), None, "unverändert gegenüber der Basis");
    assert!(
        matches!(store.save_human_on(work, 2, "x", "Autor", 42).await, Err(StoreError::UnknownRevision { .. })),
        "eine unbekannte Basis wird abgelehnt"
    );
}

#[tokio::test]
async fn save_human_takes_the_current_version_and_add_proposal_stays_proposed() {
    let (_dir, store, work) = fresh().await;
    store.save_human(work, 4, "== Erste Anzeichen\n\nNeu.\n", "Autor").await.unwrap().unwrap();
    let number = store.add_proposal(work, 4, "== Erste Anzeichen\n\nVorschlag.\n", "@lektor", "m", "n").await.unwrap();
    assert_eq!(
        rows(store.pool(), 4).await,
        vec![
            (1, None, "superseded".into()),
            (2, Some(1), "superseded".into()),
            (3, Some(2), "current".into()),
            (number as i64, Some(3), "proposed".into())
        ],
        "Speichern auf der aktuellen Fassung, Vorschlag auf der aktuellen und nur proposed"
    );
}

#[tokio::test]
async fn accepting_a_stale_proposal_keeps_every_version() {
    let (_dir, store, work) = fresh().await;
    // Der Vorschlag 3 beruht auf 2; danach speichert der Mensch eine eigene Fassung 4.
    store.save_human(work, 2, "= Einleitung\n\nEigene neue Fassung.\n", "Autor").await.unwrap().unwrap();
    store.accept(work, 2, 3).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 2),
        vec![
            (1, RevisionStatus::Superseded),
            (2, RevisionStatus::Superseded),
            (3, RevisionStatus::Current),
            (4, RevisionStatus::Superseded)
        ],
        "die eigene Fassung 4 bleibt erhalten und ist mit einem Klick wieder umschaltbar"
    );
    assert!(after.nodes[2].revision(4).unwrap().body.contains("Eigene neue Fassung"), "Text der Fassung 4 bleibt");
}

// ---- Ändern ---------------------------------------------------------------------------------------

#[tokio::test]
async fn update_revision_changes_body_and_note_and_sets_edited() {
    let (_dir, store, work) = fresh().await;
    let edit = RevisionEdit { body: Some("= Einleitung\r\n\r\nGeändert.\r\n"), note: Some("vom Menschen") };
    let change = store.update_revision(work, 2, 3, &edit, "Autor").await.unwrap().unwrap();
    assert_eq!((change.rev, change.kind), (3, ChangeKind::Updated), "Change nennt die betroffene Fassung");
    assert_eq!(change.author, Author::Human { name: "Autor".into() }, "geändert hat der Mensch");
    let before = change.before.expect("Stand vorher fehlt");
    assert_eq!(before.number, 3, "der Stand vorher gehört zur geänderten Fassung 3");
    assert_eq!(before.status, RevisionStatus::Proposed, "Status vorher");
    assert!(before.body.contains("Notrufnummern"), "alter Text im Stand vorher: {}", before.body);
    let after = store.load_work(work).await.unwrap();
    let rev = after.nodes[2].revision(3).unwrap();
    assert_eq!(rev.body, "= Einleitung\n\nGeändert.\n", "neuer Text, CRLF → LF");
    assert_eq!(rev.note, "vom Menschen", "neue Notiz");
    assert_eq!(rev.status, RevisionStatus::Proposed, "Status bleibt");
    assert!(matches!(rev.author, Author::Assistant { .. }), "Herkunft bleibt die Persona");
    let tree = store.revision_tree(work).await.unwrap();
    let info = tree[2].revisions.iter().find(|r| r.number == 3).unwrap();
    assert!(info.edited.is_some(), "edited gesetzt: {info:?}");
    assert!(tree[2].revisions.iter().filter(|r| r.number != 3).all(|r| r.edited.is_none()), "nur Fassung 3 geändert");
}

#[tokio::test]
async fn update_revision_without_change_does_nothing_and_empty_text_is_refused() {
    let (_dir, store, work) = fresh().await;
    let body = store.load_work(work).await.unwrap().nodes[0].revisions[0].body.clone();
    let same = RevisionEdit { body: Some(&body), note: Some("") };
    assert_eq!(store.update_revision(work, 0, 1, &same, "Autor").await.unwrap(), None, "gleicher Text und Notiz");
    let nothing = RevisionEdit::default();
    assert_eq!(store.update_revision(work, 0, 1, &nothing, "Autor").await.unwrap(), None, "nichts angegeben");
    let empty = RevisionEdit { body: Some("  \n"), note: None };
    let result = store.update_revision(work, 0, 1, &empty, "Autor").await;
    assert!(matches!(result, Err(StoreError::Refused(m)) if m == "Eine Fassung braucht Text."), "{result:?}");
    let tree = store.revision_tree(work).await.unwrap();
    assert!(tree[0].revisions[0].edited.is_none(), "ohne Änderung bleibt edited leer");
}

// ---- Löschen --------------------------------------------------------------------------------------

#[tokio::test]
async fn deleting_a_middle_version_reparents_its_children() {
    let (_dir, store, work) = fresh().await;
    // Einleitung: 1 ← 2 (current) ← 3 (Vorschlag); erst auf 3 umschalten, dann 2 löschen.
    store.set_current(work, 2, 3).await.unwrap();
    let change = store.delete_revision(work, 2, 2, "Autor").await.unwrap();
    assert_eq!((change.rev, change.kind), (2, ChangeKind::Deleted), "Change nennt die gelöschte Fassung");
    assert_eq!(change.reparented, vec![3], "Fassung 3 hing an 2");
    assert_eq!(change.author, Author::Human { name: "Autor".into() }, "gelöscht hat der Mensch");
    let before: RevisionRecord = change.before.expect("gelöschter Stand fehlt");
    assert_eq!(
        (before.number, before.based_on, before.status),
        (2, Some(1), RevisionStatus::Superseded),
        "gelöschter Stand: Nummer, Elternteil und Status wie vor dem Löschen (nach dem Umschalten ersetzt)"
    );
    assert!(before.body.contains("in Beziehungen"), "vollständiger alter Text: {}", before.body);
    assert_eq!(
        rows(store.pool(), 2).await,
        vec![(1, None, "superseded".into()), (3, Some(1), "current".into())],
        "Kind hängt am Elternteil der gelöschten Fassung"
    );
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(statuses(&loaded, 2), vec![(1, RevisionStatus::Superseded), (3, RevisionStatus::Current)], "Lücke");
    let saved = store.save_human(work, 2, "= Einleitung\n\nNach der Lücke.\n", "Autor").await.unwrap().unwrap();
    assert_eq!(saved.rev, 4, "die Nummer 2 wird nicht neu vergeben");
}

#[tokio::test]
async fn deleting_the_current_version_is_refused_and_changes_nothing() {
    let (_dir, store, work) = fresh().await;
    let before = store.load_work(work).await.unwrap();
    let result = store.delete_revision(work, 2, 2, "Autor").await;
    assert!(
        matches!(&result, Err(StoreError::Refused(m)) if m.starts_with("Die aktuelle Fassung lässt sich nicht löschen")),
        "{result:?}"
    );
    let after = store.load_work(work).await.unwrap();
    assert_eq!(serde_json::to_value(&after).unwrap(), serde_json::to_value(&before).unwrap(), "nichts geändert");
    assert!(
        matches!(store.delete_revision(work, 2, 7, "Autor").await, Err(StoreError::UnknownRevision { .. })),
        "eine unbekannte Fassung lässt sich nicht löschen"
    );
}

#[tokio::test]
async fn deleting_a_root_makes_its_children_roots() {
    let (_dir, store, work) = fresh().await;
    let change = store.delete_revision(work, 2, 1, "Autor").await.unwrap();
    assert_eq!(change.reparented, vec![2], "Fassung 2 hing an der Wurzel 1");
    assert_eq!(
        rows(store.pool(), 2).await,
        vec![(2, None, "current".into()), (3, Some(2), "proposed".into())],
        "das Kind der Wurzel wird selbst Wurzel"
    );
}

// ---- Baum -----------------------------------------------------------------------------------------

#[tokio::test]
async fn the_revision_tree_lists_every_section_and_version_in_order() {
    let (_dir, store, work) = fresh().await;
    let tree = store.revision_tree(work).await.unwrap();
    assert_eq!(
        tree.iter().map(|n| n.node).collect::<Vec<_>>(),
        (0..11).collect::<Vec<_>>(),
        "alle Abschnitte nach seq"
    );
    let intro = &tree[2];
    assert_eq!(
        intro.revisions.iter().map(|r| (r.number, r.based_on)).collect::<Vec<_>>(),
        [(1, None), (2, Some(1)), (3, Some(2))],
        "Einleitung: Kette 1 ← 2 ← 3 nach Nummer"
    );
    assert_eq!(
        intro.revisions[2].author,
        Author::Assistant { persona: "@lektor".into(), model: "google/gemma-4-26b-a4b".into() },
        "Autor und Modell der Fassung"
    );
    assert!(intro.revisions.iter().all(|r| r.created.starts_with("20")), "Zeitpunkt der Anlage");
    assert!(
        matches!(store.revision_tree(work + 9).await, Err(StoreError::UnknownWork(_))),
        "der Baum eines unbekannten Werks ist ein Fehler, keine leere Liste"
    );
}
