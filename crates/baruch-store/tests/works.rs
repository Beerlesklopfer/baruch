// SPDX-License-Identifier: AGPL-3.0-or-later
//! Managing works (contract Round 4, sections 1 and 3b) on a real SQLite file in the temp directory: unique titles,
//! creating from the basic framework, metadata, revisions of the work template, deleting whole works, backfill of the
//! title keys on opening.

use baruch_core::{NodeKind, RevisionStatus, WorkKind};
use baruch_store::{ChatScope, NewWork, Preset, Store, StoreError, WorkEdit};

const BOOK: &str = include_str!("data/probebuch.typ");

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn new_work(title: &str, preset: Preset) -> NewWork<'_> {
    let kind = if preset == Preset::Sermon { WorkKind::Sermon } else { WorkKind::Book };
    NewWork { title, author: "Erika Mustermann", kind, preset, subtitle: "", audience: "", bible: "" }
}

async fn count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn creating_a_book_gives_a_template_and_a_first_chapter() {
    let (_dir, store) = fresh().await;
    let new = NewWork {
        subtitle: "Ein Versuch",
        audience: "Gemeinde",
        bible: "Luther 2017",
        ..new_work("  Mein   Buch ", Preset::Book)
    };
    let created = store.create_work(&new, "Autor").await.unwrap();
    assert_eq!(
        (created.title.as_str(), created.slug.as_str()),
        ("Mein Buch", "mein-buch"),
        "Titel in einer Zeile, slug daraus"
    );
    assert_eq!(created.nodes, 1, "ein Kapitel");

    let work = store.load_work(created.work).await.unwrap();
    let outline: Vec<(&str, NodeKind, &str)> =
        work.nodes.iter().map(|n| (n.title.as_str(), n.kind, n.number.as_str())).collect();
    assert_eq!(outline, [("Erstes Kapitel", NodeKind::Chapter, "1")], "Buch: ein Kapitel „Erstes Kapitel“");
    let rev = &work.nodes[0].revisions[0];
    assert_eq!(
        (rev.number, rev.status, rev.body.as_str()),
        (1, RevisionStatus::Current, "= Erstes Kapitel\n\n"),
        "Revision 1 aktuell"
    );
    assert_eq!(rev.author, baruch_core::Author::Human { name: "Autor".into() }, "vom Menschen");

    let meta = store.work_meta(created.work).await.unwrap();
    assert_eq!(
        meta.template,
        format!(
            "#set document(title: \"Mein Buch\", author: \"Erika Mustermann\")\n#set heading(numbering: \"1.1\")\n\n{}",
            baruch_typst::cover::imprint_template()
        ),
        "Werkvorlage eines Buchs mit Dokument und Nummerierung, danach die Impressum-Vorlage (Runde 27)"
    );
    assert_eq!(
        (meta.subtitle.as_str(), meta.audience.as_str(), meta.bible.as_str()),
        ("Ein Versuch", "Gemeinde", "Luther 2017"),
        "Metadaten gespeichert"
    );
    assert_eq!(
        (meta.kind, meta.template_rev, meta.source_name),
        (WorkKind::Book, 1, None),
        "Buch, Vorlage in Fassung 1, nicht importiert"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(created.work).await.unwrap()).into_owned(),
        format!("{}= Erstes Kapitel\n\n", meta.template),
        "Export = Vorlage + Kapitel, also wieder gültiges Typst"
    );
}

#[tokio::test]
async fn creating_a_sermon_gives_text_introduction_three_points_application_and_conclusion() {
    let (_dir, store) = fresh().await;
    let created = store.create_work(&new_work("Pfingstpredigt", Preset::Sermon), "Autor").await.unwrap();
    let work = store.load_work(created.work).await.unwrap();
    assert_eq!(work.kind, WorkKind::Sermon, "Art Predigt");
    let titles: Vec<&str> = work.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Predigttext", "Hinführung", "Beobachtung am Text", "Deutung", "Zuspruch", "Anwendung", "Ausblick"],
        "Predigtvorlage (Runde 16a): Predigttext, Einleitung, drei Hauptpunkte, Anwendung, Schluss in dieser Reihenfolge"
    );
    let numbers: Vec<&str> = work.nodes.iter().map(|n| n.number.as_str()).collect();
    assert_eq!(
        numbers,
        ["", "Einleitung", "I.", "II.", "III.", "", "Schluss"],
        "Schema sermon zählt nur die gezählten Punkte: Predigttext und Anwendung ohne Nummer"
    );
    let numbered: Vec<i64> = sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = ? ORDER BY seq")
        .bind(created.work)
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert_eq!(numbered, [0, 1, 1, 1, 1, 0, 1], "„Nummer aus“ steht ausdrücklich in node.numbered");
    let scheme: String = sqlx::query_scalar("SELECT numbering_scheme FROM work WHERE id = ?")
        .bind(created.work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(scheme, "sermon", "die Predigt bekommt das Gliederungsschema sermon");
    assert!(work.nodes.iter().all(|n| n.parent.is_none() && n.kind == NodeKind::Chapter), "alle auf oberster Ebene");
    assert_eq!(
        work.nodes[2].revisions[0].body, "= Beobachtung am Text\n\n",
        "Text je Predigtteil ist seine Überschrift"
    );
    assert_eq!(work.nodes[0].revisions[0].body, "= Predigttext\n\n", "ohne Predigttext nur die Überschrift");
    let meta = store.work_meta(created.work).await.unwrap();
    assert_eq!(
        meta.template, "#set document(title: \"Pfingstpredigt\", author: \"Erika Mustermann\")\n\n",
        "Predigt: nur #set document"
    );
}

#[tokio::test]
async fn the_sermon_text_stands_under_the_heading_of_the_first_point_and_a_book_ignores_it() {
    let (_dir, store) = fresh().await;
    let quote = "#quote(block: true, attribution: [Ps 1,1 (Erfunden)])[Wohl dem Kiesel am Bach.]";
    let created =
        store.create_work_with_text(&new_work("Kieselpredigt", Preset::Sermon), "Autor", Some(quote)).await.unwrap();
    let work = store.load_work(created.work).await.unwrap();
    assert_eq!(
        work.nodes[0].revisions[0].body,
        format!("= Predigttext\n\n{quote}\n\n"),
        "der Predigttext steht unter der Überschrift des ersten Punkts, genau einmal"
    );
    assert_eq!(work.nodes[1].revisions[0].body, "= Hinführung\n\n", "die übrigen Punkte bleiben nur Überschrift");
    let book = store.create_work_with_text(&new_work("Kieselbuch", Preset::Book), "Autor", Some(quote)).await.unwrap();
    let book = store.load_work(book.work).await.unwrap();
    assert_eq!(
        book.nodes[0].revisions[0].body, "= Erstes Kapitel\n\n",
        "beim Buch gibt es keinen Predigttext; der Text landet nirgends"
    );
    assert_eq!(book.nodes[0].number, "1", "das Buch zählt wie bisher ab 1");
}

#[tokio::test]
async fn the_chat_scope_defaults_to_the_chapter_and_is_kept_in_store_meta() {
    let (_dir, store) = fresh().await;
    assert_eq!(store.chat_scope().await.unwrap(), ChatScope::Chapter, "ohne Zeile: das ganze Kapitel wie bisher");
    store.set_chat_scope(ChatScope::Section).await.unwrap();
    assert_eq!(store.chat_scope().await.unwrap(), ChatScope::Section, "gespeichert und wieder gelesen");
    let raw: String = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'chat_scope'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(raw, "section", "steht als Wort in store_meta (keine eigene Tabelle, keine Migration)");
    sqlx::query("UPDATE store_meta SET value = 'quatsch' WHERE key = 'chat_scope'")
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(store.chat_scope().await.unwrap(), ChatScope::Chapter, "ein unbekannter Wert fällt auf die Vorgabe");
}

#[tokio::test]
async fn titles_are_unique_ignoring_case_and_whitespace() {
    let (_dir, store) = fresh().await;
    let first = store.create_work(&new_work(" Das Buch ", Preset::Book), "Autor").await.unwrap();
    let before = count(&store, "SELECT count(*) FROM work").await;
    let err = store.create_work(&new_work("das  buch", Preset::Book), "Autor").await.unwrap_err();
    assert!(
        matches!(&err, StoreError::DuplicateTitle { work, slug, title } if *work == first.work && slug == "das-buch" && title == "Das Buch"),
        "„ Das Buch “ und „das  buch“ gelten als gleich: {err:?}"
    );
    assert_eq!(count(&store, "SELECT count(*) FROM work").await, before, "nichts angelegt");
    assert!(
        matches!(store.create_work(&new_work(" \n ", Preset::Book), "Autor").await, Err(StoreError::EmptyTitle)),
        "leerer Titel wird abgelehnt"
    );
    assert_eq!(StoreError::EmptyTitle.to_string(), "Ein Werk braucht einen Titel.", "deutsche Meldung");

    // An import with the title of a created work is a duplicate as well.
    let err = store.import_typst("= Eins\n", "Das Buch.typ", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::DuplicateTitle { .. }), "Titel aus dem Dateinamen ist vergeben: {err:?}");
}

#[tokio::test]
async fn updating_changes_only_what_differs_and_keeps_titles_unique() {
    let (_dir, store) = fresh().await;
    let other = store.create_work(&new_work("Anderes Werk", Preset::Book), "Autor").await.unwrap();
    let work = store.create_work(&new_work("Mein Werk", Preset::Book), "Autor").await.unwrap().work;
    let slug = store.work_slug(work).await.unwrap();

    let edit = WorkEdit {
        title: Some("Mein neues Werk"),
        subtitle: Some("Untertitel"),
        author: Some("Erika Mustermann"),
        kind: Some(WorkKind::Sermon),
        audience: Some("Jugend"),
        bible: Some("Elberfelder"),
        ..WorkEdit::default()
    };
    let update = store.update_work(work, &edit, "Autor").await.unwrap();
    assert_eq!(
        update.changed,
        ["title", "subtitle", "kind", "audience", "bible"],
        "der Autor war schon so: nicht geändert"
    );
    assert_eq!(update.template_rev, None, "Vorlage nicht berührt");
    let meta = store.work_meta(work).await.unwrap();
    assert_eq!((meta.title.as_str(), meta.kind), ("Mein neues Werk", WorkKind::Sermon), "Titel und Art geändert");
    assert_eq!(meta.slug, slug, "der Ordnername bleibt beim Umbenennen");
    assert!(meta.updated.is_some(), "updated ist gesetzt");

    let same = store.update_work(work, &edit, "Autor").await.unwrap();
    assert!(same.changed.is_empty(), "gleiche Werte ändern nichts: {same:?}");

    let case_only = WorkEdit { title: Some("MEIN NEUES WERK"), ..WorkEdit::default() };
    assert_eq!(
        store.update_work(work, &case_only, "Autor").await.unwrap().changed,
        ["title"],
        "eigener Titel in anderer Schreibung ist erlaubt"
    );

    let taken = WorkEdit { title: Some(" anderes werk "), ..WorkEdit::default() };
    let err = store.update_work(work, &taken, "Autor").await.unwrap_err();
    assert!(
        matches!(err, StoreError::DuplicateTitle { work: w, .. } if w == other.work),
        "Titel eines anderen Werks: {err:?}"
    );
    let empty = WorkEdit { title: Some("  "), subtitle: Some("verloren"), ..WorkEdit::default() };
    assert!(
        matches!(store.update_work(work, &empty, "Autor").await, Err(StoreError::EmptyTitle)),
        "leerer Titel abgelehnt"
    );
    assert_eq!(
        store.work_meta(work).await.unwrap().subtitle,
        "Untertitel",
        "abgelehnte Änderung lässt auch die übrigen Felder"
    );
}

#[tokio::test]
async fn a_changed_template_becomes_a_new_revision_and_the_old_one_stays_readable() {
    let (_dir, store) = fresh().await;
    let work = store.create_work(&new_work("Vorlagenbuch", Preset::Book), "Autor").await.unwrap().work;
    let old = store.work_meta(work).await.unwrap().template;
    let new_template = "#set text(lang: \"de\")\r\n\r\n";
    let edit = WorkEdit { template: Some(new_template), note: Some("Sprache gesetzt"), ..WorkEdit::default() };
    let update = store.update_work(work, &edit, "Autor").await.unwrap();
    assert_eq!((update.changed.as_slice(), update.template_rev), (&["template"][..], Some(2)), "neue Fassung 2");
    let meta = store.work_meta(work).await.unwrap();
    assert_eq!(meta.template, "#set text(lang: \"de\")\n\n", "work.template ist die neue Fassung, CRLF → LF");
    assert_eq!(meta.template_rev, 2, "aktuelle Fassung 2");
    assert_eq!(
        store.template_revision(work, 1).await.unwrap().as_deref(),
        Some(old.as_str()),
        "Fassung 1 bleibt lesbar"
    );
    assert_eq!(store.template_revision(work, 9).await.unwrap(), None, "unbekannte Fassung → None");
    let list = store.template_revisions(work).await.unwrap();
    assert_eq!(
        list.iter().map(|r| (r.number, r.note.as_str())).collect::<Vec<_>>(),
        [(1, "Werk angelegt"), (2, "Sprache gesetzt")],
        "Fassungsliste"
    );
    assert!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap())
            .into_owned()
            .starts_with("#set text(lang: \"de\")\n\n= Erstes Kapitel"),
        "Export nutzt die neue Vorlage"
    );

    let again = store
        .update_work(work, &WorkEdit { template: Some("#set text(lang: \"de\")\n\n"), ..WorkEdit::default() }, "Autor")
        .await;
    assert_eq!(again.unwrap().template_rev, None, "gleicher Text → keine neue Fassung");
    let heading = WorkEdit { template: Some("#set text(lang: \"de\")\n= Kapitel\n"), ..WorkEdit::default() };
    let err = store.update_work(work, &heading, "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::Refused(_)), "eine Überschrift gehört nicht in die Vorlage: {err:?}");
    assert_eq!(store.template_revisions(work).await.unwrap().len(), 2, "abgelehnt: keine Fassung dazu");
}

#[tokio::test]
async fn deleting_needs_the_typed_title_and_removes_everything_of_that_work_only() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    // Some life in the work: proposal, own revision, post with reference.
    store.add_proposal(book.work, 1, "= Hauptteil\n\nVorschlag.\n", "@lektor", "test", "").await.unwrap();
    store.save_human(book.work, 2, "== Erster Gang\n\nNeu.\n", "Autor").await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Schön.".into(),
        sources: vec![],
        refs: vec![baruch_core::TextRef { node: 0, rev: 1, quote: "Kurz".into() }],
    };
    store.add_post(book.work, 0, &post).await.unwrap();
    let sample_before = serde_json::to_value(store.load_work(sample).await.unwrap()).unwrap();

    let err = store.delete_work(book.work, "Das Probebuc").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Der eingegebene Titel passt nicht; nichts gelöscht.",
        "falscher Titel: deutsche Meldung"
    );
    assert!(store.load_work(book.work).await.is_ok(), "nach falschem Titel ist das Werk noch da");

    let deleted = store.delete_work(book.work, "  das probebuch ").await.unwrap();
    assert_eq!(
        (deleted.slug.as_str(), deleted.title.as_str(), deleted.nodes, deleted.revisions),
        ("das-probebuch", "Das Probebuch", 6, 8),
        "Bericht nennt, was gelöscht wurde"
    );
    assert!(matches!(store.load_work(book.work).await, Err(StoreError::UnknownWork(_))), "Werk ist weg");
    for table in [
        "node WHERE work_id NOT IN (SELECT id FROM work)",
        "revision WHERE node_id NOT IN (SELECT id FROM node)",
        "post WHERE node_id NOT IN (SELECT id FROM node)",
        "post_ref WHERE post_id NOT IN (SELECT id FROM post)",
        "work_template_revision WHERE work_id NOT IN (SELECT id FROM work)",
    ] {
        assert_eq!(count(&store, &format!("SELECT count(*) FROM {table}")).await, 0, "keine Reste: {table}");
    }
    assert_eq!(
        serde_json::to_value(store.load_work(sample).await.unwrap()).unwrap(),
        sample_before,
        "das andere Werk bleibt unberührt"
    );

    // Delete the work with the smallest id: the next one becomes the default.
    let next = store.create_work(&new_work("Nächstes", Preset::Book), "Autor").await.unwrap();
    store.delete_work(sample, "Beispielwerk: Leise Abhängigkeit (Auszug)").await.unwrap();
    assert_eq!(store.default_work().await.unwrap(), next.work, "Vorgabewerk ist jetzt das nächste");
    store.delete_work(next.work, "Nächstes").await.unwrap();
    assert!(
        matches!(store.default_work().await, Err(StoreError::UnknownWork(0))),
        "ohne Werke gibt es kein Vorgabewerk"
    );
    assert!(store.work_list().await.unwrap().is_empty(), "leere Werkliste");
}

#[tokio::test]
async fn the_work_list_counts_sections_and_open_proposals() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let list = store.work_list().await.unwrap();
    let entry = list.iter().find(|w| w.id == sample).unwrap();
    let work = store.load_work(sample).await.unwrap();
    let parts = work.nodes.iter().filter(|n| n.kind == NodeKind::Part).count();
    assert!(parts > 0, "Testaufbau: das Beispielwerk hat Teile");
    assert_eq!(entry.sections, work.nodes.len() - parts, "Abschnitte ohne Teile");
    assert_eq!(
        entry.proposals,
        work.nodes.iter().map(|n| n.open_proposals()).sum::<usize>(),
        "offene Vorschläge des Werks"
    );
    assert_eq!(entry.kind, WorkKind::Book, "Art");
    let updated = entry.updated.clone().expect("zuletzt geändert aus den Revisionen");
    assert!(updated >= entry.created, "zuletzt geändert nicht vor angelegt: {updated} / {}", entry.created);
}

#[tokio::test]
async fn reopening_fills_missing_title_keys_and_resolves_old_doubles_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let book = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let third = store.create_work(&new_work("Drittes", Preset::Book), "Autor").await.unwrap();
    // Reproduce the state before 0005: no keys, two works with the title of the import.
    sqlx::query("UPDATE work SET title_key = NULL").execute(store.pool()).await.unwrap();
    sqlx::query("UPDATE work SET title = 'Das Probebuch' WHERE id <> ?")
        .bind(book.work)
        .execute(store.pool())
        .await
        .unwrap();
    drop(store);

    let store = Store::open(dir.path()).await.unwrap();
    let titles: Vec<(i64, String)> = store.works().await.unwrap().into_iter().map(|(id, _, t)| (id, t)).collect();
    let sample = titles[0].0;
    assert_eq!(
        titles,
        [
            (sample, "Beispielwerk: Leise Abhängigkeit (Auszug)".into()),
            (book.work, "Das Probebuch".into()),
            (third.work, "Das Probebuch (2)".into())
        ],
        "gesätes Werk weicht dem Import und heißt wieder Beispielwerk; sonst bekommt die größere id „(2)“"
    );
    let keys: i64 = count(&store, "SELECT count(*) FROM work WHERE title_key IS NULL").await;
    assert_eq!(keys, 0, "alle Schlüssel nachgetragen");
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    let again: Vec<(i64, String)> = store.works().await.unwrap().into_iter().map(|(id, _, t)| (id, t)).collect();
    assert_eq!(again, titles, "zweites Öffnen ändert nichts");
}

// ---- Seed and work ids (contract Round 5) ------------------------------------------------------

#[tokio::test]
async fn a_new_database_is_seeded_once_with_parts_that_carry_no_text() {
    let (dir, store) = fresh().await;
    let seeded: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'seeded'")
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(seeded.as_deref(), Some("1"), "die Saat setzt store_meta.seeded in derselben Transaktion");
    let sample = store.default_work().await.unwrap();
    let work = store.load_work(sample).await.unwrap();
    let parts: Vec<(&str, &str)> = work
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Part)
        .map(|n| (n.title.as_str(), n.revisions[n.current_index().unwrap()].body.as_str()))
        .collect();
    assert_eq!(
        parts,
        [("Teil I – Erkennen", ""), ("Teil II – Lösen", "")],
        "Teile ohne „= Teil …“-Zeile, wie outline.add bei Teilen"
    );
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(sample).await.unwrap()).into_owned();
    assert!(!exported.contains("= Teil"), "Teile erscheinen im Export nicht als Kapitel: {exported}");
    let chapters = baruch_typst::parse(&exported).unwrap().headings.iter().filter(|h| h.level == 1).count();
    assert_eq!(chapters, 5, "Vorwort und vier Kapitel, keine Teile");
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(store.works().await.unwrap().len(), 1, "ein zweites Öffnen sät nicht noch einmal");
}

#[tokio::test]
async fn after_deleting_every_work_a_restart_does_not_seed_again() {
    let (dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    store.delete_work(sample, "Beispielwerk: Leise Abhängigkeit (Auszug)").await.unwrap();
    assert!(store.works().await.unwrap().is_empty(), "Testaufbau: kein Werk mehr");
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert!(
        store.works().await.unwrap().is_empty(),
        "Neustart ohne Werke: kein neues Beispielwerk (store_meta.seeded)"
    );
    let personas = count(&store, "SELECT count(*) FROM persona").await;
    assert_eq!(personas, 6, "die Personas bleiben, sie gehören zu keinem Werk");
    let created = store.create_work(&new_work("Neubeginn", Preset::Book), "Autor").await.unwrap();
    assert!(created.work > sample, "die id des gelöschten Beispielwerks kommt nicht wieder: {}", created.work);
}

#[tokio::test]
async fn deleted_works_keep_their_id_and_slug_reserved() {
    let (_dir, store) = fresh().await;
    let first = store.create_work(&new_work("Wiederkehr", Preset::Book), "Autor").await.unwrap();
    store.delete_work(first.work, "Wiederkehr").await.unwrap();
    let retired: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, slug, title FROM work_retired").fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        retired,
        [(first.work, "wiederkehr".to_owned(), "Wiederkehr".to_owned())],
        "Löschen trägt id, slug und Titel ein"
    );
    let second = store.create_work(&new_work("Wiederkehr", Preset::Book), "Autor").await.unwrap();
    assert_eq!(second.work, first.work + 1, "neue id statt der gelöschten");
    assert_eq!(second.slug, "wiederkehr-2", "der slug des gelöschten Werks bleibt reserviert");
    store.delete_work(second.work, "Wiederkehr").await.unwrap();
    let third = store.import_typst_titled(BOOK, "probebuch.typ", "Autor", "", Some("Wiederkehr")).await.unwrap();
    assert_eq!(third.slug, "wiederkehr-3", "auch der Import meidet alle reservierten slugs");
    assert_eq!(third.work, first.work + 2, "und zählt die id weiter");
}
