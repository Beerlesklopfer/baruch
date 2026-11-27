// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migrations 0104 (contract Round 3, section 2) and 0105 (contract Round 4, section 1): the full-text index
//! follows changed and deleted revisions as well as deleted and moved posts. Real SQLite file in the temp folder
//! with the core schema from the fixture; the triggers that migration 0004 of baruch-store removes ("append only")
//! are removed here as well, so that the test gets by without a dependency on baruch-store. The interplay with
//! `Store::open` is checked by the server tests (`tests/revisions.rs`).

use std::str::FromStr;

use baruch_library::{HitKind, SearchOptions, migrate, search};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const CORE_SCHEMA: &str = include_str!("fixtures/core_schema.sql");

async fn open() -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    // Like migration 0004 of the store: changing and deleting are allowed for the human.
    sqlx::raw_sql("DROP TRIGGER revision_no_delete; DROP TRIGGER revision_immutable;").execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO work (id, slug, kind, title, author) VALUES (1, 'beispiel', 'book', 'Beispielbuch', 'Autor');
         INSERT INTO node (id, work_id, seq, parent_seq, position, number, title, kind)
           VALUES (10, 1, 0, NULL, 0, '1', 'Einleitung', 'chapter');
         INSERT INTO revision (id, node_id, number, body, author_kind, author_name, status) VALUES
           (100, 10, 1, 'Der Apfelbaum blüht.', 'human', 'Autor', 'superseded'),
           (101, 10, 2, 'Die Birne reift.', 'human', 'Autor', 'current');",
    )
    .execute(&pool)
    .await
    .unwrap();
    (dir, pool)
}

fn all_revisions() -> SearchOptions {
    SearchOptions { kinds: Some(vec![HitKind::Revision]), work: Some(1), all_revisions: true, limit: None }
}

async fn index_rows(pool: &SqlitePool, id: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM search_fts WHERE kind = 'revision' AND row_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_changed_revision_text_replaces_the_old_text_in_the_index() {
    let (_dir, pool) = open().await;
    assert_eq!(search(&pool, "Birne", &all_revisions()).await.unwrap().len(), 1, "Ausgangstext ist im Index");
    sqlx::query("UPDATE revision SET body = 'Die Quitte duftet.' WHERE id = 101").execute(&pool).await.unwrap();
    let new = search(&pool, "Quitte", &all_revisions()).await.unwrap();
    assert_eq!(new.len(), 1, "neuer Text wird gefunden: {new:?}");
    assert_eq!(new[0].rev, Some(2), "Treffer zeigt auf dieselbe Fassung");
    assert!(search(&pool, "Birne", &all_revisions()).await.unwrap().is_empty(), "alter Text ist aus dem Index");
    assert_eq!(index_rows(&pool, 101).await, 1, "genau eine Indexzeile je Revision, kein Duplikat");
}

#[tokio::test]
async fn a_note_change_leaves_the_index_alone() {
    let (_dir, pool) = open().await;
    sqlx::query("UPDATE revision SET note = 'nur die Notiz' WHERE id = 101").execute(&pool).await.unwrap();
    assert_eq!(search(&pool, "Birne", &all_revisions()).await.unwrap().len(), 1, "Text bleibt im Index");
    assert!(search(&pool, "Notiz", &all_revisions()).await.unwrap().is_empty(), "Notizen sind nicht im Index");
}

#[tokio::test]
async fn a_deleted_revision_disappears_from_the_index() {
    let (_dir, pool) = open().await;
    assert_eq!(index_rows(&pool, 100).await, 1, "Fassung 1 ist im Index");
    sqlx::query("DELETE FROM revision WHERE id = 100").execute(&pool).await.unwrap();
    assert_eq!(index_rows(&pool, 100).await, 0, "Indexzeile der gelöschten Fassung ist weg");
    assert!(
        search(&pool, "Apfelbaum", &all_revisions()).await.unwrap().is_empty(),
        "gelöschter Text wird nicht gefunden"
    );
    assert_eq!(search(&pool, "Birne", &all_revisions()).await.unwrap().len(), 1, "andere Fassungen bleiben");
}

// ---- Posts (migration 0105) ---------------------------------------------------------------------------

const LIBRARY_0101: &str = include_str!("../migrations/0101_library.sql");
const SEARCH_0102: &str = include_str!("../migrations/0102_search.sql");
const HIGHLIGHTS_0103: &str = include_str!("../migrations/0103_highlights.sql");
const REVISION_INDEX_0104: &str = include_str!("../migrations/0104_revision_index.sql");

fn posts() -> SearchOptions {
    SearchOptions { kinds: Some(vec![HitKind::Post]), work: None, all_revisions: false, limit: None }
}

/// Second work with one section and two posts in the first section of work 1.
async fn insert_posts(pool: &SqlitePool) {
    sqlx::raw_sql(
        "INSERT INTO work (id, slug, kind, title, author) VALUES (2, 'predigt', 'sermon', 'Predigt', 'Autor');
         INSERT INTO node (id, work_id, seq, parent_seq, position, number, title, kind)
           VALUES (20, 2, 0, NULL, 0, '1', 'Hauptteil', 'chapter');
         INSERT INTO post (id, node_id, position, author, body) VALUES
           (500, 10, 0, 'Autor', 'Die Kastanie fällt.'),
           (501, 10, 1, '@lektor', 'Der Holunder blüht.');",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn post_rows(pool: &SqlitePool, id: i64) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM search_fts WHERE kind = 'post' AND row_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_deleted_post_disappears_from_the_search() {
    let (_dir, pool) = open().await;
    insert_posts(&pool).await;
    assert_eq!(search(&pool, "Kastanie", &posts()).await.unwrap().len(), 1, "Beitrag ist nach dem Anlegen im Index");
    sqlx::query("DELETE FROM post WHERE id = 500").execute(&pool).await.unwrap();
    assert_eq!(post_rows(&pool, 500).await, 0, "Indexzeile des gelöschten Beitrags ist weg");
    assert!(search(&pool, "Kastanie", &posts()).await.unwrap().is_empty(), "gelöschter Beitrag wird nicht gefunden");
    assert_eq!(search(&pool, "Holunder", &posts()).await.unwrap().len(), 1, "der andere Beitrag bleibt");
}

#[tokio::test]
async fn a_moved_post_carries_title_and_work_of_its_new_section() {
    let (_dir, pool) = open().await;
    insert_posts(&pool).await;
    sqlx::query("UPDATE post SET node_id = 20, position = 0 WHERE id = 501").execute(&pool).await.unwrap();
    let hits = search(&pool, "Holunder", &posts()).await.unwrap();
    assert_eq!(hits.len(), 1, "verschobener Beitrag bleibt genau einmal im Index: {hits:?}");
    assert_eq!(hits[0].work, Some(2), "Treffer gehört jetzt zum Werk des neuen Abschnitts");
    assert_eq!(hits[0].node, Some(0), "Treffer zeigt auf den neuen Abschnitt (seq 0 in Werk 2)");
    let (title, work): (String, i64) =
        sqlx::query_as("SELECT title, work_id FROM search_fts WHERE kind = 'post' AND row_id = 501")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((title.as_str(), work), ("Hauptteil", 2), "Indexzeile trägt Titel und Werk des neuen Abschnitts");
    let only_work_1 = SearchOptions { work: Some(1), ..posts() };
    assert!(
        search(&pool, "Holunder", &only_work_1).await.unwrap().is_empty(),
        "im alten Werk wird der Beitrag nicht mehr gefunden"
    );
}

#[tokio::test]
async fn migration_0105_runs_on_a_database_from_0101_to_0104_and_removes_orphans() {
    // Existing database in the state 0101–0104 (verbatim copies, same checksum).
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-migrations");
    std::fs::create_dir(&old).unwrap();
    for (name, text) in [
        ("0101_library.sql", LIBRARY_0101),
        ("0102_search.sql", SEARCH_0102),
        ("0103_highlights.sql", HIGHLIGHTS_0103),
        ("0104_revision_index.sql", REVISION_INDEX_0104),
    ] {
        std::fs::write(old.join(name), text).unwrap();
    }
    let mut before = sqlx::migrate::Migrator::new(old.as_path()).await.unwrap();
    before.set_ignore_missing(true);
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    sqlx::raw_sql("DROP TRIGGER revision_no_delete; DROP TRIGGER revision_immutable;").execute(&pool).await.unwrap();
    before.run(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO work (id, slug, kind, title, author) VALUES (1, 'beispiel', 'book', 'Beispielbuch', 'Autor');
         INSERT INTO node (id, work_id, seq, parent_seq, position, number, title, kind)
           VALUES (10, 1, 0, NULL, 0, '1', 'Einleitung', 'chapter');
         INSERT INTO revision (id, node_id, number, body, author_kind, author_name, status)
           VALUES (101, 10, 1, 'Die Birne reift.', 'human', 'Autor', 'current');",
    )
    .execute(&pool)
    .await
    .unwrap();
    insert_posts(&pool).await;
    // In the old state the index row stays behind (orphaned) when a post is deleted.
    sqlx::query("DELETE FROM post WHERE id = 500").execute(&pool).await.unwrap();
    assert_eq!(post_rows(&pool, 500).await, 1, "vor 0105 bleibt die Indexzeile des gelöschten Beitrags");
    let index_before: i64 = sqlx::query_scalar("SELECT count(*) FROM search_fts").fetch_one(&pool).await.unwrap();

    migrate(&pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    assert!(versions.starts_with(&[101, 102, 103, 104, 105]), "0105 kommt zu 0101–0104 dazu: {versions:?}");
    assert_eq!(post_rows(&pool, 500).await, 0, "verwaiste Indexzeile entfernt");
    let index_after: i64 = sqlx::query_scalar("SELECT count(*) FROM search_fts").fetch_one(&pool).await.unwrap();
    assert_eq!(index_after, index_before - 1, "nur die verwaiste Zeile fällt weg, alles andere bleibt im Index");
    assert!(search(&pool, "Kastanie", &posts()).await.unwrap().is_empty(), "der gelöschte Beitrag wird nicht gefunden");
    assert_eq!(search(&pool, "Holunder", &posts()).await.unwrap().len(), 1, "der lebende Beitrag bleibt auffindbar");
    assert_eq!(search(&pool, "Birne", &all_revisions()).await.unwrap().len(), 1, "Revisionen bleiben im Index");
    // Afterwards the new triggers take effect.
    sqlx::query("DELETE FROM post WHERE id = 501").execute(&pool).await.unwrap();
    assert_eq!(post_rows(&pool, 501).await, 0, "nach 0105 entfernt der Trigger die Indexzeile sofort");
    migrate(&pool).await.unwrap();
}
