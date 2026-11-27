// SPDX-License-Identifier: AGPL-3.0-or-later
//! Highlights (migration 0103, `library_highlight`): checks of the database, human and persona, index, deleting
//! with the title, Typst quote. Real SQLite file in the temp folder with the core schema from
//! `tests/fixtures/core_schema.sql`.

use std::str::FromStr;
use std::time::Duration;

use baruch_library::{
    HighlightAuthor, HighlightError, HighlightKind, HighlightStatus, HitKind, LibraryError, NewHighlight, SearchHit,
    SearchOptions, add_highlight, add_quote, cite_typst, delete_highlight, get_highlight, list_highlights, list_titles,
    migrate, search, seed_if_empty, update_highlight,
};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

const CORE_SCHEMA: &str = include_str!("fixtures/core_schema.sql");
const LIBRARY_0101: &str = include_str!("../migrations/0101_library.sql");
const SEARCH_0102: &str = include_str!("../migrations/0102_search.sql");

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

/// Core schema, library migrated, sample titles.
async fn library() -> Db {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    assert!(seed_if_empty(&pool).await.unwrap(), "Beispieltitel fehlen");
    Db { _dir: dir, pool }
}

/// id of the sample title `luther1912` and of its quote 1. Kor 13,4.
async fn luther(pool: &SqlitePool) -> (i64, i64) {
    let titles = list_titles(pool).await.unwrap();
    let t = titles.iter().find(|t| t.title.key == "luther1912").expect("Lutherbibel fehlt");
    let q = t.quotes.iter().find(|q| q.page == "1. Kor 13,4").expect("1. Kor 13,4 fehlt");
    (t.title.id, q.id)
}

fn human(title: i64, quote: Option<i64>, text: &str) -> NewHighlight {
    NewHighlight {
        title_id: title,
        quote_id: quote,
        locator: String::new(),
        start: None,
        end: None,
        text: text.into(),
        kind: HighlightKind::Core,
        note: String::new(),
        author: HighlightAuthor::Human { name: "Autor".into() },
    }
}

fn persona(title: i64, text: &str) -> NewHighlight {
    NewHighlight {
        author: HighlightAuthor::Persona { handle: "@lektor".into(), model: "qwen3:4b".into() },
        ..human(title, None, text)
    }
}

async fn found(pool: &SqlitePool, query: &str) -> Vec<SearchHit> {
    let options = SearchOptions { kinds: Some(vec![HitKind::Highlight]), ..Default::default() };
    search(pool, query, &options).await.unwrap()
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn migration_0103_runs_on_a_database_from_0101_and_0102() {
    // Existing database: only 0101 and 0102 (verbatim copies, same checksum), sample titles.
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-migrations");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join("0101_library.sql"), LIBRARY_0101).unwrap();
    std::fs::write(old.join("0102_search.sql"), SEARCH_0102).unwrap();
    let mut before = sqlx::migrate::Migrator::new(old.as_path()).await.unwrap();
    before.set_ignore_missing(true);
    let pool = open(dir.path()).await;
    sqlx::raw_sql(CORE_SCHEMA).execute(&pool).await.unwrap();
    before.run(&pool).await.unwrap();
    assert!(seed_if_empty(&pool).await.unwrap(), "Beispieltitel auf dem alten Stand");
    let titles_before = count(&pool, "SELECT count(*) FROM library_title").await;

    migrate(&pool).await.unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    let expected: Vec<i64> = baruch_library::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(versions, expected, "alle Bibliotheksmigrationen genau einmal (neue dürfen folgen)");
    assert!(versions.starts_with(&[101, 102, 103, 104]), "0103 und 0104 kommen dazu, 0101 und 0102 bleiben");
    assert_eq!(count(&pool, "SELECT count(*) FROM library_title").await, titles_before, "Titel bleiben erhalten");
    let (title, quote) = luther(&pool).await;
    let id = add_highlight(&pool, &human(title, Some(quote), "Die Liebe ist langmütig")).await.unwrap();
    assert!(id > 0, "neue Tabelle nimmt Hervorhebungen an");
    migrate(&pool).await.unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM search_fts WHERE kind = 'highlight'").await,
        1,
        "zweiter Lauf ändert nichts, keine Dubletten im Index"
    );
}

#[tokio::test]
async fn the_database_refuses_active_persona_highlights_empty_text_and_bad_ranges() {
    let db = library().await;
    let (title, _) = luther(&db.pool).await;
    let insert = |author_kind: &'static str,
                  model: Option<&'static str>,
                  status: &'static str,
                  text: &'static str,
                  start: Option<i64>,
                  end: Option<i64>| {
        let pool = db.pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO library_highlight (title_id, text, kind, author_kind, author_name, model, status, start_char, end_char) \
                 VALUES (?1, ?2, 'core', ?3, 'x', ?4, ?5, ?6, ?7)",
            )
            .bind(title)
            .bind(text)
            .bind(author_kind)
            .bind(model)
            .bind(status)
            .bind(start)
            .bind(end)
            .execute(&pool)
            .await
        }
    };
    assert!(insert("human", None, "active", "gut", None, None).await.is_ok(), "gültige Zeile des Menschen");
    assert!(insert("assistant", Some("m"), "proposed", "gut", None, None).await.is_ok(), "Persona als Vorschlag");
    assert!(
        insert("assistant", Some("m"), "active", "gut", None, None).await.is_err(),
        "Persona darf nicht aktiv sein"
    );
    assert!(insert("assistant", None, "proposed", "gut", None, None).await.is_err(), "Persona ohne Modell");
    assert!(insert("human", None, "active", "  \n\t ", None, None).await.is_err(), "leerer Text (nur Leerraum)");
    assert!(insert("human", None, "active", "gut", Some(5), Some(2)).await.is_err(), "Anfang hinter dem Ende");
    assert!(insert("human", None, "active", "gut", Some(-1), Some(2)).await.is_err(), "negativer Anfang");
    assert!(insert("human", None, "active", "gut", Some(1), None).await.is_err(), "nur ein Ende des Bereichs");
    assert!(
        insert("human", None, "active", "gut", Some(2), Some(2)).await.is_ok(),
        "leerer Bereich an einer Stelle ist erlaubt"
    );
}

#[tokio::test]
async fn a_human_highlight_is_active_and_a_persona_highlight_only_a_proposal() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    let mine = NewHighlight { start: Some(4), end: Some(9), ..human(title, Some(quote), "Liebe") };
    let id = add_highlight(&db.pool, &mine).await.unwrap();
    let h = get_highlight(&db.pool, id).await.unwrap();
    assert_eq!(h.status, HighlightStatus::Active, "Mensch → aktiv");
    assert_eq!(h.locator, "1. Kor 13,4", "ohne Fundstelle gilt die Stelle des Zitats");
    assert_eq!((h.start, h.end, h.quote_id), (Some(4), Some(9), Some(quote)), "Bereich und Zitat bleiben");
    let whole: String = sqlx::query_scalar("SELECT text FROM library_quote WHERE id = ?1")
        .bind(quote)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let len = whole.chars().count() as i64;
    let all = NewHighlight { start: Some(0), end: Some(len), ..human(title, Some(quote), &whole) };
    assert!(add_highlight(&db.pool, &all).await.is_ok(), "ganzes Zitat bis zum letzten Zeichen");
    let umlaut = NewHighlight { start: Some(14), end: Some(23), ..human(title, Some(quote), "langmütig") };
    assert!(add_highlight(&db.pool, &umlaut).await.is_ok(), "Bereich in Unicode-Zeichen, Umlaut zählt eins");

    let id = add_highlight(&db.pool, &persona(title, "Gotteswille")).await.unwrap();
    let h = get_highlight(&db.pool, id).await.unwrap();
    assert_eq!(h.status, HighlightStatus::Proposed, "Persona → immer Vorschlag");
    assert_eq!(
        h.author,
        HighlightAuthor::Persona { handle: "@lektor".into(), model: "qwen3:4b".into() },
        "Handle und Modell gespeichert"
    );
    let json = serde_json::to_value(&h).unwrap();
    assert_eq!(json["author"]["kind"], "persona", "Urheber getaggt serialisiert: {json}");
    assert_eq!(json["status"], "proposed", "eine Persona-Hervorhebung bleibt Vorschlag: {json}");
    assert_eq!(json["kind"], "core", "Art als Text serialisiert: {json}");

    // Setting it active fails at the database, rejected works.
    let active = update_highlight(&db.pool, id, None, None, Some(HighlightStatus::Active)).await;
    assert!(
        matches!(active, Err(HighlightError::Library(LibraryError::Db(_)))),
        "Persona bleibt Vorschlag: {active:?}"
    );
    let rejected = update_highlight(&db.pool, id, None, None, Some(HighlightStatus::Rejected)).await.unwrap();
    assert_eq!(rejected.status, HighlightStatus::Rejected, "Vorschlag lässt sich ablehnen");
}

#[tokio::test]
async fn invalid_input_gives_its_own_highlight_errors() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    let empty = add_highlight(&db.pool, &human(title, None, " \n ")).await;
    assert!(matches!(empty, Err(HighlightError::Empty)), "leerer Text: {empty:?}");
    let range =
        add_highlight(&db.pool, &NewHighlight { start: Some(3), end: Some(1), ..human(title, None, "x") }).await;
    assert!(matches!(range, Err(HighlightError::InvalidRange)), "Bereich rückwärts: {range:?}");
    let half = add_highlight(&db.pool, &NewHighlight { start: Some(3), ..human(title, None, "x") }).await;
    assert!(matches!(half, Err(HighlightError::InvalidRange)), "halber Bereich: {half:?}");
    // Range in the quote: end not behind the quote, text exactly the excerpt.
    let quote_text: String = sqlx::query_scalar("SELECT text FROM library_quote WHERE id = ?1")
        .bind(quote)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let len = quote_text.chars().count() as i64;
    let beyond = add_highlight(
        &db.pool,
        &NewHighlight { start: Some(0), end: Some(9999), ..human(title, Some(quote), "ganz anderer Text") },
    )
    .await;
    assert!(matches!(beyond, Err(HighlightError::InvalidRange)), "Ende hinter dem Zitat: {beyond:?}");
    let one_more = add_highlight(
        &db.pool,
        &NewHighlight { start: Some(0), end: Some(len + 1), ..human(title, Some(quote), &quote_text) },
    )
    .await;
    assert!(matches!(one_more, Err(HighlightError::InvalidRange)), "ein Zeichen zu weit: {one_more:?}");
    let other = add_highlight(
        &db.pool,
        &NewHighlight { start: Some(0), end: Some(3), ..human(title, Some(quote), "ganz anderer Text") },
    )
    .await;
    assert!(
        matches!(&other, Err(HighlightError::TextMismatch(excerpt)) if excerpt == "Die"),
        "Text ungleich dem Ausschnitt: {other:?}"
    );
    let empty_range =
        add_highlight(&db.pool, &NewHighlight { start: Some(2), end: Some(2), ..human(title, Some(quote), "x") }).await;
    assert!(matches!(empty_range, Err(HighlightError::TextMismatch(_))), "leerer Bereich mit Text: {empty_range:?}");
    let unknown = add_highlight(&db.pool, &human(9999, None, "x")).await;
    assert!(
        matches!(unknown, Err(HighlightError::Library(LibraryError::UnknownTitle(9999)))),
        "unbekannter Titel: {unknown:?}"
    );
    let other_title: i64 = sqlx::query_scalar("SELECT id FROM library_title WHERE id <> ?1 LIMIT 1")
        .bind(title)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let foreign = add_highlight(&db.pool, &human(other_title, Some(quote), "x")).await;
    assert!(
        matches!(foreign, Err(HighlightError::Unknown(q)) if q == quote),
        "Zitat eines anderen Titels: {foreign:?}"
    );
    assert!(matches!(get_highlight(&db.pool, 4242).await, Err(HighlightError::Unknown(4242))), "unbekannte id");
    assert!(matches!(delete_highlight(&db.pool, 4242).await, Err(HighlightError::Unknown(4242))), "Löschen unbekannt");
    assert!(
        matches!(update_highlight(&db.pool, 4242, None, Some("x"), None).await, Err(HighlightError::Unknown(4242))),
        "Ändern unbekannt"
    );
    assert_eq!(count(&db.pool, "SELECT count(*) FROM library_highlight").await, 0, "nichts angelegt");
}

#[tokio::test]
async fn highlights_are_listed_changed_and_deleted() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    let a = add_highlight(&db.pool, &human(title, Some(quote), "langmütig")).await.unwrap();
    let b = add_highlight(&db.pool, &human(title, None, "Baruch, dem Schreiber")).await.unwrap();
    let all = list_highlights(&db.pool, None).await.unwrap();
    assert_eq!(all.iter().map(|h| h.id).collect::<Vec<_>>(), vec![a, b], "mit Zitat zuerst, dann ohne");
    assert!(list_highlights(&db.pool, Some(9999)).await.unwrap().is_empty(), "fremder Titel hat keine");

    let changed =
        update_highlight(&db.pool, a, Some(HighlightKind::Contradiction), Some(" Gegen These 2 "), None).await.unwrap();
    assert_eq!(changed.kind, HighlightKind::Contradiction, "Art geändert");
    assert_eq!(changed.note, "Gegen These 2", "Notiz getrimmt");
    assert_eq!(changed.status, HighlightStatus::Active, "Status bleibt ohne Angabe");
    assert_eq!(HighlightKind::Contradiction.label(), "Widerspruch", "Anzeige auf Deutsch");

    delete_highlight(&db.pool, b).await.unwrap();
    assert_eq!(list_highlights(&db.pool, Some(title)).await.unwrap().len(), 1, "eine bleibt");
}

#[tokio::test]
async fn the_index_follows_highlights_and_skips_rejected_ones() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    let id = add_highlight(
        &db.pool,
        &NewHighlight { note: "Kleinod der Geduld".into(), ..human(title, Some(quote), "Die Liebe ist langmütig") },
    )
    .await
    .unwrap();
    let hits = found(&db.pool, "Kleinod").await;
    assert_eq!(hits.len(), 1, "Notiz ist durchsuchbar: {hits:?}");
    let hit = &hits[0];
    assert_eq!(hit.kind, HitKind::Highlight, "Art highlight");
    assert_eq!(
        (hit.highlight_id, hit.quote_id, hit.title_id),
        (Some(id), Some(quote), Some(title)),
        "Verweise im Treffer"
    );
    assert_eq!(hit.key.as_deref(), Some("luther1912"), "Zitierschlüssel der Quelle");
    assert!(
        hit.label.contains("Lutherbibel 1912") && hit.label.ends_with("1. Kor 13,4"),
        "Label mit Fundstelle: {}",
        hit.label
    );
    assert!(hit.snippet.contains("<mark>Kleinod</mark>"), "Treffer markiert: {}", hit.snippet);
    let json = serde_json::to_value(hit).unwrap();
    assert_eq!(json["kind"], "highlight", "serialisiert als highlight: {json}");
    assert_eq!(json["highlight_id"], id, "der Treffer nennt die Hervorhebung: {json}");
    assert_eq!(json["quote_id"], quote, "und ihr Zitat: {json}");

    // A quote hit carries its own id as quote_id, no highlight_id.
    let quotes =
        search(&db.pool, "langmütig", &SearchOptions { kinds: Some(vec![HitKind::Quote]), ..Default::default() })
            .await
            .unwrap();
    let q = quotes.iter().find(|h| h.quote_id == Some(quote)).expect("Zitat-Treffer mit quote_id");
    assert!(q.highlight_id.is_none(), "Zitat ist keine Hervorhebung");

    // Change: the index follows.
    update_highlight(&db.pool, id, None, Some("Perle"), None).await.unwrap();
    assert!(found(&db.pool, "Kleinod").await.is_empty(), "alte Notiz aus dem Index");
    assert_eq!(found(&db.pool, "Perle").await.len(), 1, "neue Notiz im Index");

    // Rejected: not in the search, but still there.
    update_highlight(&db.pool, id, None, None, Some(HighlightStatus::Rejected)).await.unwrap();
    assert!(found(&db.pool, "Perle").await.is_empty(), "abgelehnte Hervorhebung fehlt in der Suche");
    assert_eq!(count(&db.pool, "SELECT count(*) FROM library_highlight").await, 1, "Zeile bleibt");

    // A persona proposal can be found.
    add_highlight(&db.pool, &persona(title, "Juwel im Vorschlag")).await.unwrap();
    assert_eq!(found(&db.pool, "Juwel").await.len(), 1, "Vorschläge stehen in der Suche");

    // Delete: out of the index.
    delete_highlight(&db.pool, id).await.unwrap();
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind = 'highlight'").await,
        1,
        "nur der Vorschlag steht noch im Index"
    );
}

#[tokio::test]
async fn deleting_the_title_deletes_its_highlights_and_their_index_rows() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    add_highlight(&db.pool, &human(title, Some(quote), "Zuckerguss")).await.unwrap();
    add_highlight(&db.pool, &persona(title, "Zuckerguss als Vorschlag")).await.unwrap();
    assert_eq!(found(&db.pool, "Zuckerguss").await.len(), 2, "beide im Index");

    // Rename the title: hits carry the new title.
    sqlx::query("UPDATE library_title SET title = 'Neue Lutherbibel' WHERE id = ?1")
        .bind(title)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(found(&db.pool, "Zuckerguss").await.iter().all(|h| h.label.contains("Neue Lutherbibel")), "Titel folgt");
    assert_eq!(found(&db.pool, "Neue Lutherbibel").await.len(), 2, "Titelspalte des Index folgt");

    // Delete the quote: the highlight stays, without a quote.
    sqlx::query("DELETE FROM library_quote WHERE id = ?1").bind(quote).execute(&db.pool).await.unwrap();
    let left = list_highlights(&db.pool, Some(title)).await.unwrap();
    assert!(left.iter().all(|h| h.quote_id.is_none()), "ON DELETE SET NULL für das Zitat: {left:?}");
    assert_eq!(left.len(), 2, "Hervorhebungen bleiben beim Löschen des Zitats");

    sqlx::query("DELETE FROM library_title WHERE id = ?1").bind(title).execute(&db.pool).await.unwrap();
    assert_eq!(count(&db.pool, "SELECT count(*) FROM library_highlight").await, 0, "CASCADE löscht die Hervorhebungen");
    assert!(found(&db.pool, "Zuckerguss").await.is_empty(), "Index folgt dem Löschen des Titels");
    assert_eq!(
        count(&db.pool, "SELECT count(*) FROM search_fts WHERE kind = 'highlight'").await,
        0,
        "keine Reste im Index"
    );
}

#[tokio::test]
async fn a_citation_is_valid_typst_with_masked_special_characters() {
    let db = library().await;
    let (title, quote) = luther(&db.pool).await;
    let tricky = "Liebe #set [x] *fett* _schräg_ $a$ <l> @k ~ // nicht\n= Überschrift\n- Liste\n1. Punkt \\ Ende";
    let id = add_highlight(&db.pool, &human(title, Some(quote), tricky)).await.unwrap();
    let typst = cite_typst(&get_highlight(&db.pool, id).await.unwrap(), "luther1912");
    assert!(
        typst.starts_with("#quote(block: true, attribution: [@luther1912[1\\. Kor 13,4]])["),
        "Kopf mit Schlüssel und Fundstelle: {typst}"
    );
    assert!(typst.ends_with("Ende]"), "Inhaltsblock schließt am Ende: {typst}");
    let inner = &typst[typst.find("])[").unwrap() + 3..typst.len() - 1];
    assert_eq!(
        inner,
        "Liebe \\#set \\[x\\] \\*fett\\* \\_schräg\\_ \\$a\\$ \\<l\\> \\@k \\~ \\/\\/ nicht\n\\= Überschrift\n\\- Liste\n1\\. Punkt \\\\ Ende",
        "jedes Zeichen mit Bedeutung ist maskiert, Zeilen bleiben"
    );
    // Without escaping no open bracket: every [ and ] in the content stands behind a backslash.
    let unmasked = inner.char_indices().filter(|(i, c)| matches!(c, '[' | ']') && !inner[..*i].ends_with('\\')).count();
    assert_eq!(unmasked, 0, "keine unmaskierte Klammer im Inhalt: {inner}");
}

#[tokio::test]
async fn a_selection_that_differs_only_in_whitespace_is_stored_as_the_exact_quote_text() {
    let db = library().await;
    let (title, _) = luther(&db.pool).await;
    // Quote with line break, double space and no-break space; the selection in the browser collapses all of that
    // into single spaces.
    let quote_text = "Die Liebe ist\nlangmütig  und\u{a0}freundlich.";
    let quote = add_quote(&db.pool, title, quote_text, "1. Kor 13,4", "").await.unwrap();
    let end = quote_text.chars().count() as i64 - 1; // without the full stop
    let id = add_highlight(
        &db.pool,
        &NewHighlight {
            start: Some(4),
            end: Some(end),
            ..human(title, Some(quote), "Liebe ist langmütig und freundlich")
        },
    )
    .await
    .expect("Leerraum-Unterschiede allein dürfen die Hervorhebung nicht verhindern");
    let stored = get_highlight(&db.pool, id).await.unwrap();
    assert_eq!(
        stored.text, "Liebe ist\nlangmütig  und\u{a0}freundlich",
        "gespeichert wird der genaue Ausschnitt des Zitats, nicht die zusammengefasste Auswahl"
    );
    assert_eq!((stored.start, stored.end), (Some(4), Some(end)), "der Bereich bleibt, wie er gewählt wurde");
    let other = add_highlight(
        &db.pool,
        &NewHighlight {
            start: Some(4),
            end: Some(end),
            ..human(title, Some(quote), "Liebe ist geduldig und freundlich")
        },
    )
    .await;
    assert!(
        matches!(&other, Err(HighlightError::TextMismatch(excerpt)) if excerpt == "Liebe ist\nlangmütig  und\u{a0}freundlich"),
        "inhaltlich anderer Text bleibt abgelehnt und die Meldung nennt den Ausschnitt: {other:?}"
    );
}

#[test]
fn check_range_compares_in_nfc_so_decomposed_umlauts_from_a_pdf_are_found() {
    use baruch_library::{FulltextCheck, check_range};
    // Round 19b: full text in NFC („ä“ as one character), quote with a decomposed umlaut („a“ + U+0308, as PDFs
    // deliver it). Without the normal form these were different character sequences and the place counted as „nicht
    // gefunden“.
    let fulltext = "Die Fähre legt am Kai an.";
    let quote = "Fa\u{308}hre";
    assert_ne!(quote, "Fähre", "Voraussetzung: das Zitat ist wirklich zerlegt");
    assert_eq!(
        check_range(fulltext, 4, 9, quote),
        FulltextCheck::Found,
        "zerlegter Umlaut im Zitat gegen NFC im Volltext (Bereich in Zeichen des Volltexts)"
    );
    // The other way round: full text decomposed, range in its characters (the „ä“ counts two there), quote in NFC.
    let decomposed = "Die Fa\u{308}hre legt am Kai an.";
    assert_eq!(check_range(decomposed, 4, 10, "Fähre"), FulltextCheck::Found, "NFC-Zitat gegen zerlegten Volltext");
    // The same decomposed full text, range counted the way a normalising browser counts it.
    assert_eq!(
        check_range(decomposed, 4, 9, "Fähre"),
        FulltextCheck::Found,
        "Bereich in Zeichen der NFC-Form wird ebenfalls versucht"
    );
    assert_eq!(
        check_range(fulltext, 4, 9, "Fahre"),
        FulltextCheck::NotFound,
        "ohne Umlaut bleibt es ein anderer Text: NFC macht nicht unscharf"
    );
}
