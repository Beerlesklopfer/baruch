// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tests of the library on a file DB with the core schema from `tests/fixtures/core_schema.sql` (verbatim contract
//! section 3). Later `baruch_store::Store::open` replaces the fixture.

use std::str::FromStr;
use std::time::Duration;

use baruch_library::{
    HitKind, LibraryError, NewTitle, SearchHit, SearchOptions, add_quote, add_title, fts_query, get_title, list_titles,
    migrate, search, seed_if_empty,
};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

const CORE_SCHEMA: &str = include_str!("fixtures/core_schema.sql");
const FRAGMENT: &str = include_str!("../../baruch-server/templates/fragments/library.html");

struct Db {
    _dir: tempfile::TempDir,
    pool: SqlitePool,
}

async fn open(dir: &std::path::Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    SqlitePoolOptions::new().max_connections(4).connect_with(options).await.unwrap()
}

/// Versions of the library migrator (0101 ff.); new migrations (0106 …) do not break the tests.
fn library_versions() -> Vec<i64> {
    baruch_library::MIGRATOR.iter().map(|m| m.version).collect()
}

/// Empty DB with the core schema only (without migration table).
async fn core_only() -> Db {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    Db { _dir: dir, pool }
}

const SIGNS_V1: &str = "== Erste Anzeichen\n\nAbhängigkeit beginnt selten laut.\n\n#quote[Die Liebe ist langmütig und freundlich.] (1. Kor 13,4)\n";
const SIGNS_V2: &str = "== Erste Anzeichen\n\nSie beginnt mit kleinen Zugeständnissen, die sich wie Liebe anfühlen.\n\n#quote[Die Liebe ist langmütig und freundlich.] (1. Kor 13,4)\n";
const SIGNS_V3: &str = "== Erste Anzeichen\n\nVorschlag: langmütig heißt nicht gleichgültig.\n";

/// Two works; in work 1 section seq 1 „2.1 Erste Anzeichen“ with revisions 1 (superseded), 2 (current), 3
/// (proposed), all with „langmütig“; in work 2 a section with „langmütig“.
async fn insert_work_data(pool: &SqlitePool) {
    sqlx::raw_sql(
        "INSERT INTO work (id, slug, kind, title, author) VALUES
           (1, 'beispiel', 'book', 'Beispielbuch', 'Autor'),
           (2, 'predigt', 'sermon', 'Predigt', 'Autor');
         INSERT INTO node (id, work_id, seq, parent_seq, position, number, title, kind) VALUES
           (10, 1, 0, NULL, 0, '1', 'Einleitung', 'chapter'),
           (11, 1, 1, 0, 0, '2.1', 'Erste Anzeichen', 'section'),
           (20, 2, 0, NULL, 0, '', 'Fremdes Werk', 'section');",
    )
    .execute(pool)
    .await
    .unwrap();
    insert_revision(pool, 10, 1, "= Einleitung\n\nDieses Buch handelt von Abhängigkeit.\n", "current", None).await;
    insert_revision(pool, 11, 1, SIGNS_V1, "superseded", None).await;
    insert_revision(pool, 11, 2, SIGNS_V2, "current", None).await;
    insert_revision(pool, 11, 3, SIGNS_V3, "proposed", Some("qwen3:4b")).await;
    insert_revision(pool, 20, 1, "Gott ist langmütig mit uns.\n", "current", None).await;
    sqlx::query(
        "INSERT INTO post (node_id, position, author, body) VALUES (11, 1, 'Autor', 'Passt das Zitat zur Geduld?')",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_revision(pool: &SqlitePool, node_id: i64, number: i64, body: &str, status: &str, model: Option<&str>) {
    let (kind, name) = if model.is_some() { ("assistant", "@lektor") } else { ("human", "Autor") };
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(node_id)
    .bind(number)
    .bind(body)
    .bind(kind)
    .bind(name)
    .bind(model)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

/// Core schema, library migrated, sample titles, work data (inserted after the migration).
async fn full() -> Db {
    let db = core_only().await;
    migrate(&db.pool).await.unwrap();
    assert!(seed_if_empty(&db.pool).await.unwrap(), "Testaufbau: eine frische Bibliothek wird gesät");
    insert_work_data(&db.pool).await;
    db
}

fn kinds(hits: &[SearchHit]) -> Vec<HitKind> {
    hits.iter().map(|h| h.kind).collect()
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

#[test]
fn fixture_is_contract_section_3() {
    // Samples from the contract; the file is a verbatim copy of section 3.
    assert!(CORE_SCHEMA.starts_with("CREATE TABLE work ("), "das Fixture beginnt mit der Tabelle work wie Abschnitt 3");
    assert!(
        CORE_SCHEMA.contains("CREATE UNIQUE INDEX revision_one_current ON revision(node_id) WHERE status = 'current';"),
        "genau eine current je Abschnitt per Teilindex"
    );
    assert!(
        CORE_SCHEMA.contains("CHECK ((author_kind = 'assistant') = (model IS NOT NULL))"),
        "Assistent genau dann mit Modell"
    );
    assert!(CORE_SCHEMA.trim_end().ends_with("PRIMARY KEY (post_id, position)\n);"), "das Fixture endet mit post_ref");
    assert!(!CORE_SCHEMA.contains("_sqlx_migrations"), "das Fixture enthält keine Buchführung von sqlx");
}

#[test]
fn fixture_matches_the_core_migration_of_the_store() {
    // The library stays without a dependency on baruch-store; in return the fixture must not deviate from the real
    // core schema. The interplay is checked by the server tests with Store::open.
    let store = include_str!("../../baruch-store/migrations/0001_core.sql");
    assert_eq!(CORE_SCHEMA, store, "tests/fixtures/core_schema.sql weicht von 0001_core.sql ab");
}

#[tokio::test]
async fn migrate_twice_without_error() {
    let db = core_only().await;
    migrate(&db.pool).await.unwrap();
    migrate(&db.pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&db.pool).await.unwrap();
    assert_eq!(versions, library_versions(), "jede Bibliotheksmigration genau einmal");
    assert!(
        versions.starts_with(&[101, 102, 103, 104, 105]),
        "Bibliothek, Suche, Hervorhebungen, Revisions- und Beitragsindex: {versions:?}"
    );
}

/// As later with baruch-store: a second migrator with version 0001 (core schema) in the same `_sqlx_migrations`;
/// both run alternately several times, neither reports the other as missing.
#[tokio::test]
async fn foreign_migrator_0001_does_not_interfere() {
    let dir = tempfile::tempdir().unwrap();
    let migrations = dir.path().join("core-migrations");
    std::fs::create_dir(&migrations).unwrap();
    std::fs::write(migrations.join("0001_core.sql"), CORE_SCHEMA).unwrap();
    let mut core = sqlx::migrate::Migrator::new(migrations.as_path()).await.unwrap();
    core.set_ignore_missing(true);

    let pool = open(dir.path()).await;
    core.run(&pool).await.unwrap();
    insert_work_data(&pool).await;
    migrate(&pool).await.unwrap();
    core.run(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    let expected: Vec<i64> = std::iter::once(1).chain(library_versions()).collect();
    assert_eq!(versions, expected, "fremde 0001 bleibt, die Bibliothek ist vollständig");
    assert!(versions.starts_with(&[1, 101, 102, 103, 104, 105]), "0101–0105 sind eingetragen: {versions:?}");
    // Work data from before the library migration is searchable.
    let hits = search(&pool, "langmütig", &SearchOptions::default()).await.unwrap();
    assert!(
        hits.iter().any(|h| h.kind == HitKind::Revision),
        "0102 baut den Index aus vorhandenen Revisionen: {hits:?}"
    );
}

#[tokio::test]
async fn seed_once_without_duplicates() {
    let db = core_only().await;
    migrate(&db.pool).await.unwrap();
    assert!(seed_if_empty(&db.pool).await.unwrap(), "die erste Saat befüllt");
    assert!(!seed_if_empty(&db.pool).await.unwrap(), "die zweite Saat legt nichts doppelt an");
    let titles = list_titles(&db.pool).await.unwrap();
    assert!((2..=3).contains(&titles.len()), "zwei bis drei Beispieltitel: {} Titel", titles.len());
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM library_title").await,
        titles.len() as i64,
        "list_titles nennt jeden Titel der Tabelle"
    );
    for t in &titles {
        assert!(!t.quotes.is_empty(), "{} ohne Zitate", t.title.key);
        assert!(
            t.title.note.contains("Gemeinfrei") && t.title.note.contains("Quelle"),
            "Beispieltitel sind gemeinfrei und nennen ihre Quelle: {}",
            t.title.note
        );
        let positions: Vec<i64> = t.quotes.iter().map(|q| q.position).collect();
        assert_eq!(positions, (1..=t.quotes.len() as i64).collect::<Vec<_>>(), "Zitate fortlaufend ab 1 nummeriert");
    }
    let quotes = count(&db.pool, "SELECT count(*) FROM library_quote").await;
    assert_eq!(
        quotes,
        titles.iter().map(|t| t.quotes.len() as i64).sum::<i64>(),
        "list_titles nennt jedes Zitat der Tabelle"
    );
    // Index: exactly one row per title and per quote.
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind = 'title'").await,
        titles.len() as i64,
        "je Titel genau eine Zeile im Volltextindex"
    );
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind = 'quote'").await,
        quotes,
        "je Zitat genau eine Zeile im Volltextindex"
    );
    // At least one quote hits the word from the sample data.
    assert!(
        titles.iter().flat_map(|t| &t.quotes).any(|q| q.text.contains("langmütig")),
        "ein Beispielzitat enthält das Wort der Beispieldaten"
    );
}

#[tokio::test]
async fn search_finds_work_and_library() {
    let db = full().await;
    let hits = search(&db.pool, "langmütig", &SearchOptions::default()).await.unwrap();
    let revision = hits
        .iter()
        .find(|h| h.kind == HitKind::Revision && h.work == Some(1) && h.node == Some(1))
        .expect("Revision im Werk");
    assert_eq!(revision.rev, Some(2), "nur die current-Revision");
    assert_eq!(revision.label, "2.1 Erste Anzeichen", "Bezeichnung aus Nummer und Titel des Abschnitts");
    assert!(
        revision.snippet.contains("<mark>langmütig</mark>"),
        "Ausschnitt mit markiertem Treffer: {}",
        revision.snippet
    );
    assert!(
        revision.title_id.is_none() && revision.key.is_none(),
        "ein Werk-Treffer hat keinen Titel und keinen Zitierschlüssel"
    );

    let quote = hits.iter().find(|h| h.kind == HitKind::Quote).expect("Zitat aus der Bibliothek");
    assert!(
        quote.work.is_none() && quote.node.is_none() && quote.rev.is_none(),
        "ein Bibliothekstreffer gehört zu keinem Werk"
    );
    assert!(quote.key.is_some() && quote.title_id.is_some(), "ein Zitat-Treffer nennt Titel und Zitierschlüssel");
    assert!(quote.snippet.contains("<mark>langmütig</mark>"), "Ausschnitt mit markiertem Treffer: {}", quote.snippet);
    assert!(quote.label.contains("1. Kor 13,4"), "Bezeichnung nennt die Stelle des Zitats: {}", quote.label);

    // Ranks ascending (bm25: smaller = better).
    assert!(hits.windows(2).all(|w| w[0].rank <= w[1].rank), "Treffer nach Rang sortiert, bester zuerst");
    // Without a number only the title.
    let foreign = hits.iter().find(|h| h.work == Some(2)).expect("Treffer aus Werk 2");
    assert_eq!(foreign.label, "Fremdes Werk", "ohne Nummer zeigt die Bezeichnung nur den Titel");
}

#[tokio::test]
async fn search_folds_case_and_diacritics() {
    let db = full().await;
    for query in ["langmutig", "LANGMÜTIG", "Langmütig"] {
        let hits = search(&db.pool, query, &SearchOptions::default()).await.unwrap();
        assert!(hits.iter().any(|h| h.kind == HitKind::Revision), "{query}: Revision");
        assert!(hits.iter().any(|h| h.kind == HitKind::Quote), "{query}: Zitat");
    }
    // „Abhangigkeit“ without umlaut finds „Abhängigkeit“.
    let hits = search(&db.pool, "abhangigkeit", &SearchOptions::default()).await.unwrap();
    assert!(hits.iter().any(|h| h.node == Some(0)), "ohne Umlaut gesucht trifft die Einleitung");
}

#[tokio::test]
async fn search_revision_status_kinds_work_limit() {
    let db = full().await;
    let only_revisions = SearchOptions { kinds: Some(vec![HitKind::Revision]), work: Some(1), ..Default::default() };
    let hits = search(&db.pool, "langmütig", &only_revisions).await.unwrap();
    assert_eq!(
        hits.iter().map(|h| h.rev.unwrap()).collect::<Vec<_>>(),
        vec![2],
        "ohne all_revisions nur die aktuelle Fassung 2"
    );

    let all = SearchOptions { all_revisions: true, ..only_revisions.clone() };
    let mut revs: Vec<u32> =
        search(&db.pool, "langmütig", &all).await.unwrap().iter().map(|h| h.rev.unwrap()).collect();
    revs.sort();
    assert_eq!(revs, vec![1, 2, 3], "superseded und proposed mit all_revisions");

    // kinds
    let quotes = SearchOptions { kinds: Some(vec![HitKind::Quote]), ..Default::default() };
    let hits = search(&db.pool, "langmütig", &quotes).await.unwrap();
    assert!(!hits.is_empty() && kinds(&hits).iter().all(|k| *k == HitKind::Quote), "kinds lässt nur Zitate durch");
    let none = SearchOptions { kinds: Some(vec![]), ..Default::default() };
    assert!(search(&db.pool, "langmütig", &none).await.unwrap().is_empty(), "leere kinds-Liste ergibt keine Treffer");
    let posts = SearchOptions { kinds: Some(vec![HitKind::Post]), ..Default::default() };
    let hits = search(&db.pool, "Geduld", &posts).await.unwrap();
    assert_eq!(hits.len(), 1, "genau ein Beitrag mit „Geduld“");
    assert_eq!(
        (hits[0].work, hits[0].node, hits[0].rev),
        (Some(1), Some(1), None),
        "der Beitrag liegt in Werk 1, Abschnitt 1, ohne Revision"
    );

    // work: a foreign work is dropped, the library stays.
    let work1 = SearchOptions { work: Some(1), ..Default::default() };
    let hits = search(&db.pool, "langmütig", &work1).await.unwrap();
    assert!(hits.iter().all(|h| h.work.is_none() || h.work == Some(1)), "mit work nur dieses Werk und die Bibliothek");
    assert!(hits.iter().any(|h| h.kind == HitKind::Quote), "die Bibliothek bleibt in der Suche mit work");
    let any = search(&db.pool, "langmütig", &SearchOptions::default()).await.unwrap();
    assert!(any.iter().any(|h| h.work == Some(2)), "ohne work auch Treffer aus Werk 2");
    let work2 = SearchOptions { work: Some(2), kinds: Some(vec![HitKind::Revision]), ..Default::default() };
    let hits = search(&db.pool, "langmütig", &work2).await.unwrap();
    assert_eq!(
        hits.iter().map(|h| (h.work, h.node)).collect::<Vec<_>>(),
        vec![(Some(2), Some(0))],
        "mit work 2 nur der Abschnitt aus Werk 2"
    );

    // limit: default 20, at most 100.
    let title = add_title(&db.pool, &new_title("viele", "Viele Zitate")).await.unwrap();
    for i in 0..120 {
        add_quote(&db.pool, title, &format!("Zählwort Nummer {i}"), "", "").await.unwrap();
    }
    let len = |o: SearchOptions| {
        let pool = db.pool.clone();
        async move { search(&pool, "zählwort", &o).await.unwrap().len() }
    };
    assert_eq!(len(SearchOptions::default()).await, 20, "Vorgabe sind 20 Treffer");
    assert_eq!(len(SearchOptions { limit: Some(5), ..Default::default() }).await, 5, "limit begrenzt");
    assert_eq!(
        len(SearchOptions { limit: Some(500), ..Default::default() }).await,
        100,
        "höchstens 100 Treffer, auch wenn mehr verlangt"
    );
}

#[tokio::test]
async fn data_before_migrate_is_indexed() {
    let db = core_only().await;
    insert_work_data(&db.pool).await;
    migrate(&db.pool).await.unwrap();
    let all = SearchOptions { all_revisions: true, ..Default::default() };
    let hits = search(&db.pool, "langmütig", &all).await.unwrap();
    assert_eq!(
        hits.iter().filter(|h| h.kind == HitKind::Revision).count(),
        4,
        "alle vier Revisionen mit „langmütig“ aus der Zeit vor der Migration"
    );
    let hits = search(&db.pool, "Geduld", &SearchOptions::default()).await.unwrap();
    assert_eq!(kinds(&hits), vec![HitKind::Post], "der Beitrag aus der Zeit vor der Migration ist im Index");
    // Revisions inserted afterwards are added through the trigger, without duplicates.
    insert_revision(&db.pool, 10, 2, "= Einleitung\n\nNeu: Sanftmut.\n", "proposed", Some("qwen3:4b")).await;
    let hits = search(&db.pool, "sanftmut", &all).await.unwrap();
    assert_eq!(
        (hits.len(), hits[0].node, hits[0].rev),
        (1, Some(0), Some(2)),
        "die neue Revision kommt über den Trigger genau einmal hinzu"
    );
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind = 'revision'").await,
        6,
        "Index ohne Dubletten: jede Revision genau einmal"
    );
}

#[test]
fn fts_query_quotes_words() {
    assert_eq!(
        fts_query("Liebe langmütig").as_deref(),
        Some("\"Liebe\" \"langmütig\""),
        "jedes Wort als Phrase in Anführungszeichen"
    );
    assert_eq!(
        fts_query("a\"b OR (").as_deref(),
        Some("\"a\"\"b\" \"OR\""),
        "Anführungszeichen verdoppelt, Operatoren entschärft"
    );
    assert_eq!(fts_query(""), None, "leere Suche ergibt keine Abfrage");
    assert_eq!(fts_query("   \t "), None, "nur Leerraum ergibt keine Abfrage");
    assert_eq!(fts_query("( ) * \"\""), None, "nur Satzzeichen ergeben keine Abfrage");
}

#[tokio::test]
async fn hostile_queries_do_not_break_fts() {
    let db = full().await;
    for query in ["a\"b OR (", "NOT", "langmütig AND", "body:langmütig", "lang*", "NEAR(a b)", "^x", "-x"] {
        let result = search(&db.pool, query, &SearchOptions::default()).await;
        assert!(result.is_ok(), "feindliche Abfrage bricht die Suche nicht ab: {query}: {result:?}");
    }
    for query in ["", "   ", "((", "\""] {
        assert!(
            matches!(search(&db.pool, query, &SearchOptions::default()).await, Err(LibraryError::EmptyQuery)),
            "ohne Wörter eine leere Suche: {query}"
        );
    }
}

fn new_title(key: &str, title: &str) -> NewTitle {
    NewTitle {
        key: key.into(),
        kind: "book".into(),
        title: title.into(),
        authors: "Muster, Max".into(),
        year: Some(1900),
        publisher: "Verlag".into(),
        place: "Ort".into(),
        url: String::new(),
        note: String::new(),
    }
}

#[tokio::test]
async fn titles_and_quotes_crud() {
    let db = full().await;
    let id = add_title(&db.pool, &new_title("muster1900", "Musterbuch")).await.unwrap();
    let q1 = add_quote(&db.pool, id, "Erstes Zitat", "S. 1", "").await.unwrap();
    let q2 = add_quote(&db.pool, id, "Zweites Zitat", "S. 2", "Notiz").await.unwrap();
    let entry = get_title(&db.pool, id).await.unwrap();
    assert_eq!(entry.title.key, "muster1900", "Zitierschlüssel gespeichert");
    assert_eq!(
        entry.quotes.iter().map(|q| (q.id, q.position)).collect::<Vec<_>>(),
        vec![(q1, 1), (q2, 2)],
        "Zitate in ihrer Reihenfolge"
    );
    assert!(
        matches!(get_title(&db.pool, 9999).await, Err(LibraryError::UnknownTitle(9999))),
        "unbekannter Titel → UnknownTitle"
    );
    assert!(
        matches!(add_quote(&db.pool, 9999, "x", "", "").await, Err(LibraryError::UnknownTitle(9999))),
        "Zitat zu unbekanntem Titel → UnknownTitle"
    );
    assert!(
        matches!(add_title(&db.pool, &new_title("muster1900", "Doppelt")).await, Err(LibraryError::Db(_))),
        "doppelter Zitierschlüssel scheitert an UNIQUE"
    );

    // list_titles: by authors, then title (empty authors first).
    let list = list_titles(&db.pool).await.unwrap();
    let keys: Vec<&str> = list.iter().map(|t| t.title.key.as_str()).collect();
    assert_eq!(keys.last(), Some(&"muster1900"), "„Muster“ steht in der Sortierung nach Autoren hinten");
    let json = serde_json::to_value(&list[0]).unwrap();
    assert!(json.get("key").is_some() && json.get("quotes").is_some(), "flach serialisiert: {json}");

    // Label of a title hit: „Autor: Titel (Jahr)“.
    let hits =
        search(&db.pool, "Musterbuch", &SearchOptions { kinds: Some(vec![HitKind::Title]), ..Default::default() })
            .await
            .unwrap();
    assert_eq!(hits.len(), 1, "genau ein Titeltreffer");
    assert_eq!(hits[0].label, "Muster, Max: Musterbuch (1900)", "Bezeichnung „Autor: Titel (Jahr)“");
    assert_eq!(
        (hits[0].title_id, hits[0].key.as_deref()),
        (Some(id), Some("muster1900")),
        "der Treffer nennt Titel und Zitierschlüssel"
    );
}

#[tokio::test]
async fn index_follows_updates_and_deletes() {
    let db = full().await;
    let id = add_title(&db.pool, &new_title("alt", "Altertitel")).await.unwrap();
    let q = add_quote(&db.pool, id, "Ein seltenes Wort: Kleinod.", "", "").await.unwrap();
    let found = |q: &'static str| {
        let pool = db.pool.clone();
        async move { search(&pool, q, &SearchOptions::default()).await.unwrap() }
    };
    assert_eq!(found("Altertitel").await.len(), 2, "Titel und Zitat (Titelspalte)");

    // Rename the title: title row and quote row carry the new title.
    sqlx::query("UPDATE library_title SET title = 'Neutitel' WHERE id = ?1").bind(id).execute(&db.pool).await.unwrap();
    assert!(found("Altertitel").await.is_empty(), "der alte Titel ist aus dem Index verschwunden");
    let hits = found("Neutitel").await;
    assert_eq!(hits.len(), 2, "Titel und Zitat tragen den neuen Titel");
    assert!(
        hits.iter().any(|h| h.kind == HitKind::Quote && h.label.contains("Neutitel")),
        "die Zitatzeile trägt den neuen Titel"
    );

    // Change the quote.
    sqlx::query("UPDATE library_quote SET text = 'Jetzt: Perle.' WHERE id = ?1")
        .bind(q)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(found("Kleinod").await.is_empty(), "der alte Zitattext ist aus dem Index verschwunden");
    assert_eq!(found("Perle").await.len(), 1, "der neue Zitattext ist im Index");

    // Delete the quote.
    sqlx::query("DELETE FROM library_quote WHERE id = ?1").bind(q).execute(&db.pool).await.unwrap();
    assert!(found("Perle").await.is_empty(), "das gelöschte Zitat ist aus dem Index verschwunden");

    // Delete the title: quotes disappear by CASCADE, also from the index.
    add_quote(&db.pool, id, "Noch ein Juwel.", "", "").await.unwrap();
    assert_eq!(found("Juwel").await.len(), 1, "das neue Zitat ist im Index");
    sqlx::query("DELETE FROM library_title WHERE id = ?1").bind(id).execute(&db.pool).await.unwrap();
    assert!(found("Neutitel").await.is_empty(), "der gelöschte Titel ist aus dem Index verschwunden");
    assert!(found("Juwel").await.is_empty(), "seine Zitate sind per CASCADE mit verschwunden");
    assert_eq!(count(&db.pool, "SELECT count(*) FROM library_quote WHERE title_id = 999999 OR title_id NOT IN (SELECT id FROM library_title)").await, 0, "keine verwaisten Zitate");
    let indexed = count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind IN ('title','quote')").await;
    let rows =
        count(&db.pool, "SELECT (SELECT count(*) FROM library_title) + (SELECT count(*) FROM library_quote)").await;
    assert_eq!(indexed, rows, "Index und Tabellen der Bibliothek stimmen überein");

    // Change the section title: work hits carry the new title.
    sqlx::query("UPDATE node SET title = 'Frühe Zeichen' WHERE id = 11").execute(&db.pool).await.unwrap();
    let hits = found("frühe zeichen").await;
    assert!(
        hits.iter().any(|h| h.kind == HitKind::Revision && h.label == "2.1 Frühe Zeichen"),
        "Werk-Treffer tragen den neuen Abschnittstitel"
    );
}

fn environment() -> minijinja::Environment<'static> {
    let mut env = minijinja::Environment::new();
    env.add_template("fragments/library.html", FRAGMENT).unwrap();
    env
}

#[tokio::test]
async fn fragment_renders_titles_and_hits() {
    let db = full().await;
    let evil = add_title(&db.pool, &new_title("boese", "<script>alert(1)</script> & Co")).await.unwrap();
    add_quote(&db.pool, evil, "Zitat mit <b>Tags</b>", "S. 3", "").await.unwrap();
    sqlx::query("UPDATE library_title SET url = 'javascript:alert(1)' WHERE id = ?1")
        .bind(evil)
        .execute(&db.pool)
        .await
        .unwrap();
    let library = list_titles(&db.pool).await.unwrap();
    let hits = search(&db.pool, "langmütig", &SearchOptions::default()).await.unwrap();
    let env = environment();
    let html = env
        .get_template("fragments/library.html")
        .unwrap()
        .render(minijinja::context! {
            library => minijinja::Value::from(minijinja::value::Serde(&library)),
            library_query => "langmütig",
            library_hits => minijinja::Value::from(minijinja::value::Serde(&hits)),
            node => minijinja::context! { id => 1 },
        })
        .unwrap();
    assert!(
        !html.contains(r#"<h2 class="pane-title">Literatur</h2>"#),
        "Überschrift gehört dem Popup bzw. der Seite: {html}"
    );
    assert!(
        html.contains(r#"<form action="/literatur" method="get" class="library-search""#),
        "Suche ohne JavaScript geht an die Seite /literatur: {html}"
    );
    assert!(html.contains(r#"<input type="search" name="lq" value="langmütig""#), "das Suchfeld behält die Suche");
    assert!(html.contains(r#"<input type="hidden" name="node" value="1">"#), "das Suchformular behält den Abschnitt");
    assert!(html.contains(r#"data-widget="library""#), "das Widget library hängt am Fragment");
    assert_eq!(html.matches("<details class=\"library-title\"").count(), library.len(), "je Titel ein Eintrag");
    for t in &library {
        assert!(html.contains(&format!(r#"data-key="{}""#, t.title.key)), "jeder Titel trägt seinen Zitierschlüssel");
        assert!(html.contains(&format!(r#"id="lit-{}""#, t.title.key)), "jeder Titel hat einen Anker");
    }
    assert_eq!(
        html.matches("<blockquote").count(),
        library.iter().map(|t| t.quotes.len()).sum::<usize>(),
        "je Zitat ein blockquote"
    );
    assert!(html.contains("1. Kor 13,4"), "die Stelle des Zitats steht im Eintrag");
    let revision = hits
        .iter()
        .find(|h| h.kind == HitKind::Revision && h.work == Some(1) && h.node == Some(1))
        .expect("Werk-Treffer");
    let key = revision.node_key.expect("Werk-Treffer tragen den festen Schlüssel (Runde 8)");
    assert!(
        html.contains(&format!("/?work=1&amp;key={key}&amp;rev=2&amp;lq=langm%C3%BCtig#fundstelle\"")),
        "Werk-Treffer verlinkt mit work, key (nicht node), rev, lq und dem Anker der Fundstelle: {html}"
    );
    assert!(!html.contains("&amp;node="), "kein erzeugter Link trägt mehr node= (Runde 8): {html}");
    assert!(
        html.contains(r#"id="lit-luther1912" data-key="luther1912" open>"#),
        "Titel mit Zitat-Treffer ist ohne JS aufgeklappt: {html}"
    );
    assert!(!html.contains(r#"name="work""#), "ohne work.id im Kontext kein verstecktes Werkfeld");
    let quote = hits.iter().find(|h| h.kind == HitKind::Quote).expect("Zitat-Treffer");
    let (title_id, quote_id) = (quote.title_id.unwrap(), quote.quote_id.unwrap());
    assert!(
        html.contains(&format!(
            "href=\"/quelle?title={title_id}&amp;quote={quote_id}&amp;lq=langm%C3%BCtig#zitat-{quote_id}\""
        )),
        "Zitat-Treffer öffnet die Dokumentansicht am Zitat mit Suche; ohne Schlüssel im Kontext keine Stelle \
         (Runde 8: kein node= in Links): {html}"
    );
    assert!(html.contains("<mark>langmütig</mark>"), "Treffer im Ausschnitt markiert");
    // Escaping
    assert!(!html.contains("<script>") && html.contains("&lt;script&gt;"), "Titel maskiert, kein <script>");
    assert!(!html.contains("<b>Tags</b>"), "Zitattext maskiert");
    assert!(html.contains("&lt;b&gt;Tags&lt;&#x2f;b&gt;"), "Zitattext maskiert: {html}");
    assert!(!html.contains("javascript:"), "nur http(s)-Links");
    assert!(!html.contains("Noch keine Titel."), "mit Titeln kein Hinweis auf eine leere Bibliothek");
}

#[tokio::test]
async fn the_fragment_keeps_work_and_encodes_the_query_in_links() {
    let db = full().await;
    let library = list_titles(&db.pool).await.unwrap();
    // Hits from a simple search, shown under an input with special characters.
    let query = "langmütig & 100%+#x";
    let hits = search(&db.pool, "langmütig", &SearchOptions::default()).await.unwrap();
    assert!(hits.iter().any(|h| h.kind == HitKind::Revision), "Testsuche braucht einen Werk-Treffer: {hits:?}");
    let html = environment()
        .get_template("fragments/library.html")
        .unwrap()
        .render(minijinja::context! {
            library => minijinja::Value::from(minijinja::value::Serde(&library)),
            library_query => query,
            library_hits => minijinja::Value::from(minijinja::value::Serde(&hits)),
            work => minijinja::context! { id => 7, title => "Beispielbuch" },
        })
        .unwrap();
    assert!(html.contains(r#"<input type="hidden" name="work" value="7">"#), "Suchformular behält das Werk: {html}");
    // Filter `urlencode`: % & # + and space are encoded, so that the value does not fall apart into further
    // parameters, the anchor or a space; the umlaut stands as UTF-8 in % form.
    assert!(
        html.contains("&amp;lq=langm%C3%BCtig%20%26%20100%25%2B%23x#fundstelle\""),
        "lq im Treffer-Link nicht richtig kodiert: {html}"
    );
}

#[tokio::test]
async fn the_fragment_without_a_query_has_no_open_titles() {
    let db = full().await;
    let library = list_titles(&db.pool).await.unwrap();
    let html = environment()
        .get_template("fragments/library.html")
        .unwrap()
        .render(minijinja::context! {
            library => minijinja::Value::from(minijinja::value::Serde(&library)),
            library_query => None::<&str>,
            library_hits => Vec::<()>::new(),
        })
        .unwrap();
    assert!(!html.contains(" open>"), "ohne Suche bleibt jeder Titel zugeklappt: {html}");
    assert!(html.contains(r#"id="lit-luther1912""#), "Beispieltitel fehlt: {html}");
}

#[tokio::test]
async fn snippets_escape_html_and_mark_hits() {
    let db = full().await;
    insert_revision(
        &db.pool,
        20,
        2,
        "<script>alert(1)</script> Quargel & \"Brot\" #emph[Quargel]\n",
        "proposed",
        Some("m"),
    )
    .await;
    let options = SearchOptions { all_revisions: true, ..Default::default() };
    let hits = search(&db.pool, "Quargel", &options).await.unwrap();
    let hit = hits.iter().find(|h| h.kind == HitKind::Revision).unwrap_or_else(|| panic!("Revision fehlt: {hits:?}"));
    assert!(!hit.snippet.contains("<script>"), "Text muss maskiert sein: {}", hit.snippet);
    assert!(hit.snippet.contains("&lt;script&gt;"), "maskiertes <script> fehlt: {}", hit.snippet);
    assert!(
        hit.snippet.contains("&amp;") && hit.snippet.contains("&quot;Brot&quot;"),
        "& und \" maskiert: {}",
        hit.snippet
    );
    assert!(
        hit.snippet.contains("#emph[<mark>Quargel</mark>]"),
        "Typst-Klammern bleiben, Treffer in <mark>: {}",
        hit.snippet
    );
    let without_marks = hit.snippet.replace("<mark>", "").replace("</mark>", "");
    assert!(!without_marks.contains('<') && !without_marks.contains('>'), "außer <mark> kein Tag: {}", hit.snippet);
    assert!(
        !hit.snippet.contains('\u{E000}') && !hit.snippet.contains('\u{E001}'),
        "interne Marken übrig: {}",
        hit.snippet
    );
}

#[tokio::test]
async fn nul_and_control_characters_in_a_query_do_not_break_fts() {
    let db = full().await;
    for query in ["\0", "\u{1}\u{2}", " \0 \u{7f} "] {
        let result = search(&db.pool, query, &SearchOptions::default()).await;
        assert!(
            matches!(result, Err(LibraryError::EmptyQuery)),
            "{query:?}: nur Steuerzeichen ergeben eine leere Suche: {result:?}"
        );
    }
    let hits = search(&db.pool, "lang\0mütig", &SearchOptions::default()).await;
    assert!(hits.is_ok(), "NUL mitten im Wort darf die Suche nicht abbrechen: {hits:?}");
    let hits = search(&db.pool, "langmütig\0", &SearchOptions::default()).await.unwrap();
    assert!(!hits.is_empty(), "NUL am Ende wird übergangen, das Wort trifft weiter");
}

#[test]
fn fragment_renders_empty_library() {
    let env = environment();
    let html = env
        .get_template("fragments/library.html")
        .unwrap()
        .render(minijinja::context! { library => Vec::<()>::new() })
        .unwrap();
    assert!(html.contains("Noch keine Titel."), "leere Bibliothek mit Hinweis");
    assert!(html.contains(r#"name="lq""#), "das Suchfeld gibt es auch ohne Titel");
    assert!(!html.contains(r#"name="node""#), "ohne node kein verstecktes Feld");
    assert!(!html.contains("library-hits"), "ohne Suche keine Trefferliste");
    assert!(html.contains(r#"data-widget="library""#), "das Widget library hängt auch an der leeren Liste");
}
