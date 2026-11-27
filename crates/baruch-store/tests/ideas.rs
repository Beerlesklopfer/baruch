// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einfälle im Store (Vertrag Runde 7, Abschnitt 6b) auf einer echten SQLite-Datei im Temp-Ordner:
//! Anlegen mit Grenzen, Lesen, Liste mit Filtern, Schlagwort-Wolke, Volltextsuche, Ändern, Löschen
//! und Wiederherstellen, Verarbeiten, `einfaelle.md` im Spiegel, Werk löschen. Testtexte frei erfunden.

use baruch_store::{
    IdeaEdit, IdeaFilter, IdeaOrigin, IdeaScope, IdeaStatus, IdeaUse, MAX_IDEA_WORDS, MAX_WORD_CHARS, NewIdea, Store,
    StoreError, WorkId,
};

const BOOK: &str = include_str!("data/probebuch.typ");

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

fn human() -> IdeaOrigin {
    IdeaOrigin::Human { name: "Autor".into() }
}

fn idea<'a>(work: Option<WorkId>, key: Option<i64>, words: &'a [String], note: &'a str) -> NewIdea<'a> {
    NewIdea { work, key, words, note, quote: "", source: "", bible: "", origin: human() }
}

/// Fester Schlüssel des Abschnitts an Stelle `seq`.
async fn key(store: &Store, work: WorkId, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn an_idea_is_created_with_cleaned_words_and_read_back() {
    let (_dir, store) = fresh().await;
    let work = store.default_work().await.unwrap();
    let k = key(&store, work, 1).await;
    let list = words(&["  Laube ", "Rosen", "laube", "Anna's   Garten"]);
    let new = NewIdea {
        quote: "Die Laube steht im Schatten.\r\n",
        source: "Literatur: Gartenbuch",
        bible: "Hld 2,12",
        ..idea(Some(work), Some(k), &list, "  Die Laube braucht ein Dach.\r\nUnd Rosen.  ")
    };
    let created = store.idea_create(&new).await.unwrap();
    assert_eq!(
        created.words,
        ["Laube", "Rosen", "Anna's Garten"],
        "getrimmt, Doppel zusammengefasst, Leerraum vereinfacht"
    );
    assert_eq!(created.note, "Die Laube braucht ein Dach.\nUnd Rosen.", "Gedanke getrimmt, CRLF → LF");
    assert_eq!(created.quote, "Die Laube steht im Schatten.", "Markierung getrimmt");
    assert_eq!((created.work, created.key), (Some(work), Some(k)), "Werk und Bezug gespeichert");
    assert!(created.node_title.is_some(), "Titel des Abschnitts im Bezug gelesen: {created:?}");
    assert_eq!(created.status, IdeaStatus::Open, "neu = offen");
    assert_eq!(created.origin, human(), "Herkunft Mensch");
    assert!(created.deleted.is_none() && created.done_as.is_none(), "weder gelöscht noch verarbeitet");
    assert_eq!(store.idea_get(created.id).await.unwrap(), created, "idea_get liefert denselben Einfall");
    let json = serde_json::to_value(&created).unwrap();
    assert_eq!(json["origin"], serde_json::json!({ "kind": "human", "name": "Autor" }), "Herkunft mit kind: {json}");
    assert_eq!(json["status"], "open", "Status klein geschrieben: {json}");
    assert!(matches!(store.idea_get(999).await, Err(StoreError::UnknownIdea(999))), "unbekannte id → UnknownIdea");
}

#[tokio::test]
async fn limits_are_refused_and_nothing_is_written() {
    let (_dir, store) = fresh().await;
    let work = store.default_work().await.unwrap();
    let long_word = "w".repeat(MAX_WORD_CHARS + 1);
    let many: Vec<String> = (0..=MAX_IDEA_WORDS).map(|i| format!("Beet{i}")).collect();
    let long_note = "n".repeat(4001);
    let long_source = "q".repeat(201);
    let cases: Vec<(Vec<String>, &str, &str, &str, &str)> = vec![
        (words(&[]), "", "", "", "kein Schlagwort"),
        (words(&["  ", ""]), "", "", "", "nur leere Schlagwörter"),
        (words(&["Rose\nDorn"]), "", "", "", "Zeilenumbruch im Schlagwort"),
        (vec![long_word], "", "", "", "Schlagwort über 60 Zeichen"),
        (many, "", "", "", "21 Schlagwörter"),
        (words(&["Laube"]), &long_note, "", "", "Gedanke über 4000 Zeichen"),
        (words(&["Laube"]), "", &long_source, "", "Quelle über 200 Zeichen"),
        (words(&["Laube"]), "", "", &long_source, "Bibelstelle über 200 Zeichen"),
    ];
    for (list, note, source, bible, what) in &cases {
        let new = NewIdea { source, bible, ..idea(Some(work), None, list, note) };
        let err = store.idea_create(&new).await.unwrap_err();
        assert!(matches!(err, StoreError::Idea(_)), "{what} → StoreError::Idea, war {err:?}");
    }
    assert_eq!(count(&store, "SELECT count(*) FROM idea").await, 0, "nichts angelegt");
    assert_eq!(count(&store, "SELECT count(*) FROM idea_fts").await, 0, "nichts im Index");

    let ok = words(&["ä".repeat(MAX_WORD_CHARS).as_str()]);
    assert!(store.idea_create(&idea(None, None, &ok, &"n".repeat(4000))).await.is_ok(), "genau an der Grenze geht es");
}

#[tokio::test]
async fn the_section_must_belong_to_the_work_and_the_work_must_exist() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap().work;
    let foreign = key(&store, book, 1).await;
    let list = words(&["Laube"]);
    let err = store.idea_create(&idea(Some(sample), Some(foreign), &list, "")).await.unwrap_err();
    assert_eq!(err.to_string(), "Der Abschnitt gehört nicht zu diesem Werk.", "Bezug aus fremdem Werk abgelehnt");
    let err = store.idea_create(&idea(None, Some(foreign), &list, "")).await.unwrap_err();
    assert!(matches!(err, StoreError::Idea(_)), "Bezug ohne Werk abgelehnt: {err:?}");
    let err = store.idea_create(&idea(Some(9999), None, &list, "")).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownWork(9999)), "unbekanntes Werk → UnknownWork: {err:?}");
    let created = store.idea_create(&idea(None, None, &list, "")).await.unwrap();
    assert_eq!((created.work, created.key), (None, None), "ohne Werk geht");
}

#[tokio::test]
async fn lists_filter_by_work_word_status_and_show_newest_first() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap().work;
    let a = store.idea_create(&idea(Some(sample), None, &words(&["Laube", "Rose"]), "eins")).await.unwrap();
    let b = store.idea_create(&idea(Some(book), None, &words(&["laube"]), "zwei")).await.unwrap();
    let c = store.idea_create(&idea(None, None, &words(&["Dorn"]), "drei")).await.unwrap();
    let ids = |list: Vec<baruch_store::Idea>| list.into_iter().map(|i| i.id).collect::<Vec<_>>();

    let all = store.idea_list(&IdeaFilter::default()).await.unwrap();
    assert_eq!(ids(all), [c.id, b.id, a.id], "alle, neueste zuerst");
    let only = IdeaFilter { scope: IdeaScope::Work(book), ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&only).await.unwrap()), [b.id], "nur das Buch");
    let none = IdeaFilter { scope: IdeaScope::NoWork, ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&none).await.unwrap()), [c.id], "nur ohne Werk");
    let laube = IdeaFilter { word: Some("LAUBE".into()), ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&laube).await.unwrap()), [b.id, a.id], "Schlagwort ohne Rücksicht auf Groß/klein");

    store.idea_mark_done(a.id, IdeaUse::Note, "").await.unwrap();
    let open = IdeaFilter { status: Some(IdeaStatus::Open), ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&open).await.unwrap()), [c.id, b.id], "nur offene");
    let done = IdeaFilter { status: Some(IdeaStatus::Done), ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&done).await.unwrap()), [a.id], "nur verarbeitete");
    let page = IdeaFilter { limit: 1, offset: 1, ..IdeaFilter::default() };
    assert_eq!(ids(store.idea_list(&page).await.unwrap()), [b.id], "limit und offset");

    let cloud = store.idea_words(&IdeaFilter { word: Some("dorn".into()), ..IdeaFilter::default() }).await.unwrap();
    let pairs: Vec<(String, u32)> = cloud.into_iter().map(|w| (w.word.to_lowercase(), w.count)).collect();
    assert_eq!(
        pairs,
        [("laube".to_owned(), 2), ("dorn".to_owned(), 1), ("rose".to_owned(), 1)],
        "Wolke über alle (ohne Wortfilter), häufigste zuerst, gleiche Schlüssel zusammen"
    );
}

#[tokio::test]
async fn search_finds_without_diacritics_escapes_snippets_and_skips_deleted_ideas() {
    let (_dir, store) = fresh().await;
    let a = store
        .idea_create(&idea(None, None, &words(&["Gärtnerin"]), "Anna's <Laube> & die Rosen blühen"))
        .await
        .unwrap();
    let b = store.idea_create(&idea(None, None, &words(&["Dorn"]), "Hecke am Zaun")).await.unwrap();
    let found = store.idea_search("\"gartnerin\"", &IdeaFilter::default()).await.unwrap();
    assert_eq!(found.iter().map(|(i, _)| i.id).collect::<Vec<_>>(), [a.id], "Akzente zählen nicht");
    let found = store.idea_search("\"laube\"", &IdeaFilter::default()).await.unwrap();
    let snippet = &found[0].1;
    assert!(snippet.contains("&lt;<mark>Laube</mark>&gt;"), "Treffer markiert, Text maskiert: {snippet}");
    assert!(snippet.contains("Anna&#39;s") && snippet.contains("&amp;"), "Sonderzeichen maskiert: {snippet}");

    store.idea_delete(b.id).await.unwrap();
    assert!(
        store.idea_search("\"hecke\"", &IdeaFilter::default()).await.unwrap().is_empty(),
        "gelöschte nicht im Index"
    );
    store.idea_restore(b.id).await.unwrap();
    assert_eq!(
        store.idea_search("\"hecke\"", &IdeaFilter::default()).await.unwrap().len(),
        1,
        "wiederhergestellt: wieder im Index"
    );

    store.idea_update(a.id, &IdeaEdit { note: Some("Nur noch Tulpen"), ..IdeaEdit::default() }).await.unwrap();
    assert!(
        store.idea_search("\"rosen\"", &IdeaFilter::default()).await.unwrap().is_empty(),
        "Index folgt der Änderung"
    );
    assert_eq!(store.idea_search("\"tulpen\"", &IdeaFilter::default()).await.unwrap().len(), 1, "neuer Text gefunden");
    let err = store.idea_search("\"offen", &IdeaFilter::default()).await.unwrap_err();
    assert!(matches!(err, StoreError::Idea(_)), "kaputter FTS-Ausdruck → Idea, kein Datenbankfehler: {err:?}");
}

#[tokio::test]
async fn deleting_hides_an_idea_restoring_brings_it_back() {
    let (_dir, store) = fresh().await;
    let created = store.idea_create(&idea(None, None, &words(&["Laube"]), "")).await.unwrap();
    let deleted = store.idea_delete(created.id).await.unwrap();
    assert!(deleted.deleted.is_some(), "deleted gesetzt");
    assert!(store.idea_list(&IdeaFilter::default()).await.unwrap().is_empty(), "nicht mehr in der Liste");
    let bin = IdeaFilter { deleted: true, ..IdeaFilter::default() };
    assert_eq!(store.idea_list(&bin).await.unwrap().len(), 1, "unter den gelöschten");
    assert!(store.idea_words(&IdeaFilter::default()).await.unwrap().is_empty(), "nicht in der Wolke");
    assert!(store.idea_get(created.id).await.unwrap().deleted.is_some(), "idea_get findet ihn weiter");
    assert!(matches!(store.idea_delete(created.id).await, Err(StoreError::Idea(_))), "zweimal löschen abgelehnt");
    let edit = IdeaEdit { note: Some("neu"), ..IdeaEdit::default() };
    assert!(matches!(store.idea_update(created.id, &edit).await, Err(StoreError::Idea(_))), "gelöschte nicht änderbar");
    assert!(
        matches!(store.idea_mark_done(created.id, IdeaUse::Post, "").await, Err(StoreError::Idea(_))),
        "gelöschte nicht verarbeitbar"
    );
    let restored = store.idea_restore(created.id).await.unwrap();
    assert!(restored.deleted.is_none(), "wiederhergestellt");
    assert_eq!(restored.words, created.words, "alles wie vorher");
    assert!(matches!(store.idea_restore(created.id).await, Err(StoreError::Idea(_))), "nicht gelöscht → abgelehnt");
}

#[tokio::test]
async fn updating_changes_only_what_is_given_and_moving_drops_the_old_section() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap().work;
    let k = key(&store, sample, 1).await;
    let created = store.idea_create(&idea(Some(sample), Some(k), &words(&["Laube"]), "alt")).await.unwrap();
    assert!(store.idea_update(created.id, &IdeaEdit::default()).await.unwrap().is_none(), "nichts angegeben → None");
    let same = IdeaEdit { note: Some(" alt "), ..IdeaEdit::default() };
    assert!(store.idea_update(created.id, &same).await.unwrap().is_none(), "gleicher Text (getrimmt) → None");

    let list = words(&["Rose", "Dorn"]);
    let edit = IdeaEdit { words: Some(&list), bible: Some("Ps 23"), ..IdeaEdit::default() };
    let changed = store.idea_update(created.id, &edit).await.unwrap().expect("geändert");
    assert_eq!(
        (changed.words.clone(), changed.bible.as_str(), changed.note.as_str()),
        (list.clone(), "Ps 23", "alt"),
        "nur Genanntes"
    );
    assert_eq!(changed.key, Some(k), "Bezug bleibt im selben Werk");

    let moved = store
        .idea_update(created.id, &IdeaEdit { work: Some(Some(book)), ..IdeaEdit::default() })
        .await
        .unwrap()
        .unwrap();
    assert_eq!((moved.work, moved.key), (Some(book), None), "neues Werk: der alte Bezug entfällt");
    let bad = IdeaEdit { key: Some(Some(k)), ..IdeaEdit::default() };
    assert!(
        matches!(store.idea_update(created.id, &bad).await, Err(StoreError::Idea(_))),
        "Bezug aus fremdem Werk abgelehnt"
    );
    let none =
        store.idea_update(created.id, &IdeaEdit { work: Some(None), ..IdeaEdit::default() }).await.unwrap().unwrap();
    assert_eq!(none.work, None, "ohne Werk");

    let done = store
        .idea_update(created.id, &IdeaEdit { status: Some(IdeaStatus::Done), ..IdeaEdit::default() })
        .await
        .unwrap()
        .unwrap();
    assert_eq!((done.status, done.done_as), (IdeaStatus::Done, Some(IdeaUse::Note)), "von Hand erledigt = Notiz");
    let processed = store.idea_mark_done(created.id, IdeaUse::Outline, "key 7").await.unwrap();
    assert_eq!(
        (processed.done_as, processed.done_ref.as_deref()),
        (Some(IdeaUse::Outline), Some("key 7")),
        "verarbeitet"
    );
    let open = store
        .idea_update(created.id, &IdeaEdit { status: Some(IdeaStatus::Open), ..IdeaEdit::default() })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (open.status, open.done_as, open.done_ref),
        (IdeaStatus::Open, None, None),
        "offen setzt Verarbeitung zurück"
    );
}

#[tokio::test]
async fn einfaelle_md_appears_in_the_mirror_only_with_ideas_and_the_export_stays_the_same() {
    let (_dir, store) = fresh().await;
    let work = store.default_work().await.unwrap();
    let before = store.mirror_snapshot(work).await.unwrap();
    let export = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(!before.iter().any(|f| f.path == "einfaelle.md"), "ohne Einfälle keine Datei");

    let k = key(&store, work, 1).await;
    let first =
        store.idea_create(&idea(Some(work), Some(k), &words(&["Laube", "Rose"]), "Zeile 1\nZeile 2")).await.unwrap();
    let dorn = words(&["Dorn"]);
    let mcp = NewIdea { origin: IdeaOrigin::Mcp { handle: "@mcp:probe".into() }, ..idea(Some(work), None, &dorn, "") };
    let second = store.idea_create(&mcp).await.unwrap();
    store.idea_create(&idea(None, None, &words(&["Zaun"]), "")).await.unwrap();
    let after = store.mirror_snapshot(work).await.unwrap();
    assert_eq!(&after[..before.len()], &before[..], "die übrigen Dateien bleiben gleich");
    let file = after.iter().find(|f| f.path == "einfaelle.md").expect("einfaelle.md im Werkspiegel");
    let text = &file.contents;
    let title = store.load_work(work).await.unwrap().title;
    assert!(text.starts_with(&format!("# Einfälle – {title}\n")), "Kopf mit Werktitel: {text}");
    let at_second = text.find(&format!("## Einfall {} · offen", second.id)).expect("zweiter Eintrag");
    let at_first = text.find(&format!("## Einfall {} · offen", first.id)).expect("erster Eintrag");
    assert!(at_second < at_first, "neueste zuerst: {text}");
    assert!(text.contains("- Schlagwörter: Laube, Rose\n- Gedanke: Zeile 1\n  Zeile 2\n"), "Werte zeilenweise: {text}");
    assert!(
        text.contains(&format!("- Bezug: Abschnitt „{}“ (key {k})", first.node_title.clone().unwrap())),
        "Bezug: {text}"
    );
    assert!(text.contains("- Herkunft: MCP @mcp:probe · "), "Herkunft MCP: {text}");
    assert!(!text.contains("Zaun"), "Einfälle ohne Werk stehen nicht im Werkspiegel: {text}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        export,
        "Export unverändert"
    );

    let loose = store.idea_mirror_file(None).await.unwrap().expect("Datei der Einfälle ohne Werk");
    assert!(
        loose.contents.starts_with("# Einfälle ohne Werk\n") && loose.contents.contains("Zaun"),
        "{}",
        loose.contents
    );

    store.idea_delete(first.id).await.unwrap();
    store.idea_delete(second.id).await.unwrap();
    let gone = store.mirror_snapshot(work).await.unwrap();
    assert_eq!(gone, before, "alle gelöscht: Spiegel wie vorher (ohne einfaelle.md)");
}

#[tokio::test]
async fn deleting_a_work_keeps_its_ideas_as_ideas_without_a_work() {
    let (_dir, store) = fresh().await;
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let k = key(&store, book.work, 2).await;
    let created = store.idea_create(&idea(Some(book.work), Some(k), &words(&["Gang"]), "")).await.unwrap();
    store.delete_work(book.work, &book.title).await.unwrap();
    let after = store.idea_get(created.id).await.unwrap();
    assert_eq!((after.work, after.key, after.node_title), (None, None, None), "Werk und Bezug entfallen (SET NULL)");
    assert_eq!(after.words, created.words, "der Einfall selbst bleibt");
    let none = IdeaFilter { scope: IdeaScope::NoWork, ..IdeaFilter::default() };
    assert_eq!(store.idea_list(&none).await.unwrap().len(), 1, "jetzt unter „ohne Werk“");
    assert_eq!(
        count(&store, "SELECT count(*) FROM pragma_foreign_key_check").await,
        0,
        "keine Verstöße gegen Fremdschlüssel"
    );
}
