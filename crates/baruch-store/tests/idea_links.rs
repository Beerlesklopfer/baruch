// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindungen und Lagen der Gedankenkarte im Store (Vertrag Runde 25 §2, §3.1) auf einer echten SQLite-Datei:
//! Ablehnungen ohne Schreiben, „widerspricht“ normalisiert, gleiches Paar bleibt bzw. kehrt zurück, weiches
//! Löschen und Wiederherstellen, gelöschte Einfälle fallen aus Listen und Lagen und kommen zurück, Lagen mit
//! Grenzen und Grabsteinen, Zeile „Verbindungen“ in `einfaelle.md`, uuid nur über den Fülltrigger.
//! Testtexte frei erfunden.

use baruch_store::{
    IdeaEdit, IdeaId, IdeaOrigin, LinkKind, MAP_COORD_LIMIT, MAX_LINK_NOTE_CHARS, NewIdea, NewIdeaLink, Store,
    StoreError, WorkId,
};

const ORCHARD: &str = "#set document(title: \"Das Obstgartenbuch\", author: \"Ilse Ast\")\n\n\
= Blüte\n\nIm April summt es.\n\n= Ernte\n\nKörbe voller Äpfel.\n";

const MEADOW: &str = "#set document(title: \"Das Heckenbuch\", author: \"Ilse Ast\")\n\n= Schlehe\n\nDornig.\n";

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn human() -> IdeaOrigin {
    IdeaOrigin::Human { name: "Autor".into() }
}

async fn idea(store: &Store, work: Option<WorkId>, words: &[&str]) -> IdeaId {
    let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    let new = NewIdea { work, key: None, words: &words, note: "", quote: "", source: "", bible: "", origin: human() };
    store.idea_create(&new).await.unwrap().id
}

fn link(from: IdeaId, to: IdeaId, kind: LinkKind, note: &str) -> NewIdeaLink<'_> {
    NewIdeaLink { from, to, kind, note, origin: human() }
}

async fn count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(store.pool()).await.unwrap()
}

fn is_v4(uuid: &str) -> bool {
    baruch_exchange::is_uuid(uuid) && &uuid[14..15] == "4"
}

/// Ein Werk mit drei Einfällen und ein zweites mit einem.
async fn two_works(store: &Store) -> (WorkId, WorkId, [IdeaId; 3], IdeaId) {
    let orchard = store.import_typst(ORCHARD, "obstgarten.typ", "Autor").await.unwrap().work;
    let meadow = store.import_typst(MEADOW, "hecke.typ", "Autor").await.unwrap().work;
    let a = idea(store, Some(orchard), &["Apfel", "Korb"]).await;
    let b = idea(store, Some(orchard), &["Biene"]).await;
    let c = idea(store, Some(orchard), &["Blüte", "Apfel"]).await;
    let d = idea(store, Some(meadow), &["Schlehe"]).await;
    (orchard, meadow, [a, b, c], d)
}

#[tokio::test]
async fn forbidden_links_are_refused_and_nothing_is_written() {
    let (_dir, store) = fresh().await;
    let (_, _, [a, b, c], d) = two_works(&store).await;
    store.idea_delete(c).await.unwrap();
    let long = "x".repeat(MAX_LINK_NOTE_CHARS + 1);
    let cases: Vec<(NewIdeaLink<'_>, &str)> = vec![
        (link(a, a, LinkKind::LeadsTo, ""), "Selbstbezug"),
        (link(a, d, LinkKind::BelongsTo, ""), "fremdes Werk"),
        (link(a, c, LinkKind::ExampleOf, ""), "gelöschter Einfall"),
        (link(c, a, LinkKind::Contradicts, ""), "gelöschter Einfall (getauscht)"),
        (link(a, b, LinkKind::LeadsTo, &long), "Notiz über 500 Zeichen"),
        (link(a, b, LinkKind::LeadsTo, "Zeile\tTab"), "Steuerzeichen in der Notiz"),
    ];
    for (new, why) in &cases {
        let result = store.idea_link_create(new).await;
        // Vertrag §2: fachliche Ablehnung über StoreError::Idea (→ -32602), wie bei Einfällen.
        assert!(matches!(result, Err(StoreError::Idea(_))), "{why}: abgelehnt mit Idea, war {result:?}");
    }
    let unknown = store.idea_link_create(&link(a, 99_999, LinkKind::LeadsTo, "")).await;
    assert!(matches!(unknown, Err(StoreError::UnknownIdea(99_999))), "unbekannter Einfall: {unknown:?}");
    let nameless =
        NewIdeaLink { origin: IdeaOrigin::Persona { handle: " ".into() }, ..link(a, b, LinkKind::LeadsTo, "") };
    assert!(store.idea_link_create(&nameless).await.is_err(), "Herkunft ohne Namen abgelehnt");
    // Nichts geschrieben: keine Zeile, auch kein halber Zustand aus einer abgebrochenen Transaktion.
    assert_eq!(count(&store, "SELECT count(*) FROM idea_link").await, 0, "keine Verbindung angelegt");
    // Exakt 500 Zeichen gehen (Grenze in Zeichen, nicht Bytes).
    let max = "ä".repeat(MAX_LINK_NOTE_CHARS);
    assert!(store.idea_link_create(&link(a, b, LinkKind::LeadsTo, &max)).await.is_ok(), "500 Zeichen erlaubt");
}

#[tokio::test]
async fn contradicts_is_stored_from_the_smaller_id_and_counts_once() {
    let (_dir, store) = fresh().await;
    let (_, _, [a, b, _], _) = two_works(&store).await;
    assert!(a < b, "Testannahme: a wurde zuerst angelegt");
    let (first, changed) = store.idea_link_create(&link(b, a, LinkKind::Contradicts, "beißt sich")).await.unwrap();
    // „widerspricht“ ist ungerichtet: gespeichert immer from < to (CHECK in 0043), egal in welcher Richtung gefragt.
    assert_eq!((first.from, first.to), (a, b), "getauscht auf from < to: {first:?}");
    assert!(changed, "neu angelegt");
    assert_eq!(first.origin, human(), "Herkunft gespeichert");
    assert!(is_v4(&first.uuid), "uuid vom Fülltrigger, kanonisch v4: {}", first.uuid);
    let (again, changed) = store.idea_link_create(&link(a, b, LinkKind::Contradicts, "anders")).await.unwrap();
    // Dieselbe Verbindung in der anderen Richtung ist kein Doppel und ändert nichts (auch nicht die Notiz).
    assert_eq!((again.id, changed), (first.id, false), "gleiche Verbindung, unverändert");
    assert_eq!(again.note, "beißt sich", "Notiz bleibt bei unverändert");
    // Gerichtete Arten sind dagegen zwei verschiedene Verbindungen je Richtung.
    let (there, _) = store.idea_link_create(&link(a, b, LinkKind::LeadsTo, "")).await.unwrap();
    let (back, _) = store.idea_link_create(&link(b, a, LinkKind::LeadsTo, "")).await.unwrap();
    assert_ne!(there.id, back.id, "führt weiter a→b und b→a sind verschieden");
    assert_eq!((back.from, back.to), (b, a), "gerichtete Art wird nicht getauscht");
    assert_eq!(count(&store, "SELECT count(*) FROM idea_link").await, 3, "drei Zeilen");
}

#[tokio::test]
async fn an_active_pair_stays_unchanged_and_a_deleted_one_comes_back() {
    let (_dir, store) = fresh().await;
    let (_, _, [a, b, _], _) = two_works(&store).await;
    let (made, _) = store.idea_link_create(&link(a, b, LinkKind::ExampleOf, "erste Notiz")).await.unwrap();
    let (same, changed) = store.idea_link_create(&link(a, b, LinkKind::ExampleOf, "")).await.unwrap();
    assert!(!changed, "Vertrag §2: aktives gleiches Paar+Art → changed false");
    assert_eq!(same, made, "nichts geändert, auch nicht updated");

    let gone = store.idea_link_delete(made.id).await.unwrap();
    assert!(gone.deleted.is_some(), "weich gelöscht");
    assert_eq!(store.idea_link_get(made.id).await.unwrap().deleted, gone.deleted, "get findet auch gelöschte");
    let twice = store.idea_link_delete(made.id).await;
    assert!(matches!(twice, Err(StoreError::Idea(ref m)) if m.contains("schon gelöscht")), "zweimal: {twice:?}");

    // Ohne neue Notiz bleibt die alte; die Verbindung ist dieselbe Zeile (uuid bleibt, kein Doppel).
    let (back, changed) = store.idea_link_create(&link(a, b, LinkKind::ExampleOf, "")).await.unwrap();
    assert!(changed && back.deleted.is_none(), "gelöscht → wiederhergestellt, changed true: {back:?}");
    assert_eq!(
        (back.id, back.uuid.as_str(), back.note.as_str()),
        (made.id, made.uuid.as_str(), "erste Notiz"),
        "dieselbe Zeile mit derselben uuid, die alte Notiz bleibt ohne neue"
    );
    // Mit neuer Notiz wird sie ersetzt.
    store.idea_link_delete(made.id).await.unwrap();
    let (renoted, _) = store.idea_link_create(&link(a, b, LinkKind::ExampleOf, "neue Notiz")).await.unwrap();
    assert_eq!(renoted.note, "neue Notiz", "Notiz ersetzt, wenn eine neue mitkommt");

    let not_deleted = store.idea_link_restore(made.id).await;
    assert!(matches!(not_deleted, Err(StoreError::Idea(_))), "nicht gelöscht → Idea: {not_deleted:?}");
    store.idea_link_delete(made.id).await.unwrap();
    let restored = store.idea_link_restore(made.id).await.unwrap();
    assert!(restored.deleted.is_none(), "idea_link_restore stellt wieder her");
    let unknown = store.idea_link_get(4711).await;
    assert!(
        matches!(unknown, Err(StoreError::Idea(ref m)) if m == "Verbindung 4711 gibt es nicht."),
        "unbekannt → Idea mit Satz aus dem Vertrag: {unknown:?}"
    );
    assert_eq!(count(&store, "SELECT count(*) FROM idea_link").await, 1, "immer dieselbe eine Zeile");
}

#[tokio::test]
async fn a_deleted_idea_drops_out_of_links_and_positions_and_returns_with_its_restore() {
    let (_dir, store) = fresh().await;
    let (orchard, _, [a, b, c], _) = two_works(&store).await;
    let (ab, _) = store.idea_link_create(&link(a, b, LinkKind::LeadsTo, "")).await.unwrap();
    let (bc, _) = store.idea_link_create(&link(b, c, LinkKind::BelongsTo, "")).await.unwrap();
    store.idea_map_pos_set(orchard, b, 10.0, -20.5).await.unwrap();
    store.idea_map_pos_set(orchard, c, 1.0, 2.0).await.unwrap();
    let ids = |links: Vec<baruch_store::IdeaLink>| links.into_iter().map(|l| l.id).collect::<Vec<_>>();
    assert_eq!(ids(store.idea_links_of_work(orchard, false).await.unwrap()), vec![ab.id, bc.id], "beide nach id");

    store.idea_delete(b).await.unwrap();
    // Vertrag §2: gelöschter Einfall – Verbindungen und Lage fallen heraus (Filter, kein Schreiben).
    assert!(store.idea_links_of_work(orchard, false).await.unwrap().is_empty(), "keine aktive Verbindung mehr");
    assert!(store.idea_links_of_idea(a, false).await.unwrap().is_empty(), "auch nicht beim Partner");
    assert_eq!(ids(store.idea_links_of_idea(a, true).await.unwrap()), vec![ab.id], "mit deleted=true noch da");
    assert_eq!(ids(store.idea_links_of_work(orchard, true).await.unwrap()), vec![ab.id, bc.id], "alle mit deleted");
    let positions = store.idea_map_positions(orchard).await.unwrap();
    assert_eq!(positions.iter().map(|p| p.idea).collect::<Vec<_>>(), vec![c], "Lage von b fällt heraus");
    assert_eq!(count(&store, "SELECT count(*) FROM idea_link WHERE deleted IS NULL").await, 2, "nichts geschrieben");
    assert_eq!(count(&store, "SELECT count(*) FROM idea_map_pos").await, 2, "Lage bleibt gespeichert");
    let refused = store.idea_link_create(&link(b, c, LinkKind::ExampleOf, "")).await;
    assert!(matches!(refused, Err(StoreError::Idea(_))), "mit gelöschtem Einfall nicht verbindbar");
    let refused = store.idea_map_pos_set(orchard, b, 0.0, 0.0).await;
    assert!(matches!(refused, Err(StoreError::Idea(_))), "gelöschter Einfall bekommt keine Lage");

    store.idea_restore(b).await.unwrap();
    assert_eq!(ids(store.idea_links_of_work(orchard, false).await.unwrap()), vec![ab.id, bc.id], "beide zurück");
    let positions = store.idea_map_positions(orchard).await.unwrap();
    assert_eq!(positions.iter().map(|p| p.idea).collect::<Vec<_>>(), vec![b, c], "Lage zurück");
    assert_eq!((positions[0].x, positions[0].y), (10.0, -20.5), "Koordinaten unverändert");
}

#[tokio::test]
async fn positions_are_checked_replaced_and_reset_with_tombstones() {
    let (_dir, store) = fresh().await;
    let (orchard, meadow, [a, b, c], d) = two_works(&store).await;
    let first = store.idea_map_pos_set(orchard, a, 120.25, -40.0).await.unwrap();
    assert_eq!((first.idea, first.x, first.y), (a, 120.25, -40.0), "Lage gespeichert");
    let uuid: String = sqlx::query_scalar("SELECT uuid FROM idea_map_pos WHERE idea_id = ?")
        .bind(a)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(is_v4(&uuid), "uuid vom Fülltrigger: {uuid}");
    let moved = store.idea_map_pos_set(orchard, a, -5.0, 7.5).await.unwrap();
    assert_eq!((moved.x, moved.y), (-5.0, 7.5), "zweites Setzen ersetzt");
    let again: String = sqlx::query_scalar("SELECT uuid FROM idea_map_pos WHERE idea_id = ?")
        .bind(a)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(again, uuid, "je (Werk, Einfall) eine Lage; Ersetzen behält die uuid (Austausch erkennt sie wieder)");
    assert_eq!(count(&store, "SELECT count(*) FROM idea_map_pos").await, 1, "eine Zeile");

    for (x, y, why) in [
        (f64::NAN, 0.0, "NaN"),
        (0.0, f64::INFINITY, "unendlich"),
        (MAP_COORD_LIMIT + 1.0, 0.0, "über der Grenze"),
        (0.0, -MAP_COORD_LIMIT - 0.5, "unter der Grenze"),
    ] {
        let refused = store.idea_map_pos_set(orchard, b, x, y).await;
        assert!(matches!(refused, Err(StoreError::Idea(_))), "{why}: abgelehnt, war {refused:?}");
    }
    assert!(store.idea_map_pos_set(orchard, b, MAP_COORD_LIMIT, -MAP_COORD_LIMIT).await.is_ok(), "Grenze selbst geht");
    let foreign = store.idea_map_pos_set(orchard, d, 0.0, 0.0).await;
    assert!(matches!(foreign, Err(StoreError::Idea(_))), "Einfall eines anderen Werks: {foreign:?}");
    store.idea_map_pos_set(orchard, c, 3.0, 3.0).await.unwrap();
    store.idea_map_pos_set(meadow, d, 9.0, 9.0).await.unwrap();

    assert_eq!(store.idea_map_pos_reset(orchard, Some(b)).await.unwrap(), 1, "eine entfernt");
    assert_eq!(store.idea_map_pos_reset(orchard, Some(b)).await.unwrap(), 0, "schon fort: 0");
    assert_eq!(store.idea_map_pos_reset(orchard, None).await.unwrap(), 2, "die übrigen zwei des Werks");
    assert_eq!(store.idea_map_positions(meadow).await.unwrap().len(), 1, "das andere Werk bleibt unberührt");
    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(orchard).fetch_one(store.pool()).await.unwrap();
    // Hartes Löschen hinterlässt Grabsteine der neuen Art mit Werk-uuid (Austausch: „hier gelöscht“).
    let graves: i64 =
        sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE kind = 'idea_map_pos' AND work_uuid = ?")
            .bind(&work_uuid)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(graves, 3, "drei Grabsteine idea_map_pos mit der uuid des Werks");
    assert!(matches!(store.idea_map_pos_reset(9999, None).await, Err(StoreError::UnknownWork(9999))), "Werk fehlt");
}

#[tokio::test]
async fn deleting_a_work_removes_its_positions_with_tombstones_and_keeps_the_links() {
    let (_dir, store) = fresh().await;
    let (orchard, _, [a, b, _], _) = two_works(&store).await;
    store.idea_map_pos_set(orchard, a, 1.0, 1.0).await.unwrap();
    let (ab, _) = store.idea_link_create(&link(a, b, LinkKind::LeadsTo, "")).await.unwrap();
    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(orchard).fetch_one(store.pool()).await.unwrap();
    store.delete_work(orchard, "Das Obstgartenbuch").await.unwrap();
    assert_eq!(count(&store, "SELECT count(*) FROM idea_map_pos").await, 0, "Lagen mit dem Werk fort");
    let grave: Option<String> = sqlx::query_scalar("SELECT work_uuid FROM uuid_tombstone WHERE kind = 'idea_map_pos'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    // work_map_pos_cascade_uuid_tombstone löscht die Lagen, solange die Werkzeile noch steht.
    assert_eq!(grave.as_deref(), Some(work_uuid.as_str()), "Grabstein kennt die Werk-uuid");
    // Einfälle werden beim Werklöschen zu Einfällen ohne Werk (0011), ihre Verbindung bleibt bestehen.
    let links = store.idea_links_of_idea(a, false).await.unwrap();
    assert_eq!(links.iter().map(|l| l.id).collect::<Vec<_>>(), vec![ab.id], "Verbindung bleibt");
}

#[tokio::test]
async fn ideas_without_a_work_can_be_linked_with_each_other_only() {
    let (_dir, store) = fresh().await;
    let (_, _, [a, ..], _) = two_works(&store).await;
    let x = idea(&store, None, &["Wolke"]).await;
    let y = idea(&store, None, &["Regen"]).await;
    let (made, _) = store.idea_link_create(&link(x, y, LinkKind::LeadsTo, "")).await.unwrap();
    assert_eq!((made.from, made.to), (x, y), "beide ohne Werk: erlaubt (Vertrag §2 work_id IS work_id)");
    let mixed = store.idea_link_create(&link(x, a, LinkKind::LeadsTo, "")).await;
    assert!(matches!(mixed, Err(StoreError::Idea(_))), "ohne Werk ↔ mit Werk abgelehnt: {mixed:?}");
}

#[tokio::test]
async fn the_mirror_file_lists_links_after_the_other_fields() {
    let (_dir, store) = fresh().await;
    let (orchard, _, [a, b, c], _) = two_works(&store).await;
    let e = idea(&store, Some(orchard), &["Leiter"]).await;
    store.idea_link_create(&link(b, a, LinkKind::LeadsTo, "")).await.unwrap(); // a ← b
    store.idea_link_create(&link(c, a, LinkKind::Contradicts, "süß\noder sauer")).await.unwrap(); // a ↔ c
    store.idea_link_create(&link(a, e, LinkKind::ExampleOf, "")).await.unwrap(); // a → e
    let gone = store.idea_link_create(&link(a, c, LinkKind::BelongsTo, "")).await.unwrap().0;
    store.idea_link_delete(gone.id).await.unwrap();
    let file = store.idea_mirror_file(Some(orchard)).await.unwrap().unwrap();
    let entry_of = |id: IdeaId| {
        let head = format!("## Einfall {id} ·");
        let start = file.contents.find(&head).unwrap_or_else(|| panic!("Eintrag {id} fehlt: {}", file.contents));
        let rest = &file.contents[start + head.len()..];
        rest[..rest.find("\n## ").unwrap_or(rest.len())].to_owned()
    };
    // Vertrag §4.1: nach Verbindungs-id; → ausgehend, ← eingehend, ↔ ungerichtet; nur aktive; Notiz in „…“.
    let expected =
        format!("- Verbindungen: ← {b} führt weiter; ↔ {c} widerspricht („süß oder sauer“); → {e} Beispiel für\n");
    let entry = entry_of(a);
    assert!(entry.contains(&expected), "Zeile für Einfall {a}:\n{entry}");
    assert!(entry.find("- Herkunft:").unwrap() < entry.find("- Verbindungen:").unwrap(), "nach den übrigen Feldern");
    assert!(entry_of(b).contains(&format!("- Verbindungen: → {a} führt weiter\n")), "Gegenrichtung bei b");
    assert!(entry_of(c).contains(&format!("- Verbindungen: ↔ {a} widerspricht („süß oder sauer“)\n")), "↔ bei c");
    // Gelöschter Partner: Zeile fällt weg (nur zu nicht gelöschten Einfällen), ohne Verbindung keine Zeile.
    store.idea_delete(e).await.unwrap();
    store.idea_delete(c).await.unwrap();
    let file = store.idea_mirror_file(Some(orchard)).await.unwrap().unwrap();
    assert!(
        file.contents.contains(&format!("- Verbindungen: ← {b} führt weiter\n")),
        "nur noch die Verbindung zu b:\n{}",
        file.contents
    );
    assert_eq!(file.contents.matches("- Verbindungen:").count(), 2, "nur a und b haben noch eine Zeile");
}

#[tokio::test]
async fn moving_a_linked_idea_to_another_work_unlinks_it_and_drops_its_positions() {
    let (_dir, store) = fresh().await;
    let (orchard, meadow, [a, b, c], _) = two_works(&store).await;
    let (ab, _) = store.idea_link_create(&link(a, b, LinkKind::LeadsTo, "")).await.unwrap();
    // a ist das Ziel: das Lösen von der Seite eines verschobenen Ziels scheiterte vorher (Befund Runde 25).
    let (ca, _) = store.idea_link_create(&link(c, a, LinkKind::ExampleOf, "")).await.unwrap();
    let (bc, _) = store.idea_link_create(&link(b, c, LinkKind::BelongsTo, "")).await.unwrap();
    store.idea_map_pos_set(orchard, a, 10.0, 20.0).await.unwrap();
    store.idea_map_pos_set(orchard, b, 30.0, 40.0).await.unwrap();
    let orchard_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(orchard).fetch_one(store.pool()).await.unwrap();

    let edit = IdeaEdit { work: Some(Some(meadow)), ..IdeaEdit::default() };
    store.idea_update(a, &edit).await.unwrap().expect("umgezogen");

    // Verbindungen gibt es nur innerhalb eines Werks: beide Verbindungen von a sind weich gelöst, die
    // Verbindung b → c, die a nicht berührt, bleibt.
    assert!(store.idea_links_of_idea(a, false).await.unwrap().is_empty(), "a hat keine aktive Verbindung mehr");
    for id in [ab.id, ca.id] {
        assert!(store.idea_link_get(id).await.unwrap().deleted.is_some(), "Verbindung {id} weich gelöst");
    }
    assert!(store.idea_link_get(bc.id).await.unwrap().deleted.is_none(), "fremde Verbindung bleibt");
    assert_eq!(
        store.idea_links_of_work(orchard, false).await.unwrap().iter().map(|l| l.id).collect::<Vec<_>>(),
        vec![bc.id],
        "im alten Werk nur noch b → c"
    );
    // einfaelle.md beider Werke ohne die Verbindung über die Werkgrenze (vorher stand sie in beiden).
    let old_file = store.idea_mirror_file(Some(orchard)).await.unwrap().unwrap().contents;
    let partners: Vec<String> = old_file
        .lines()
        .filter_map(|l| l.strip_prefix("- Verbindungen: "))
        .flat_map(|l| l.split("; ").map(|part| part.split(' ').nth(1).unwrap_or_default().to_owned()))
        .collect();
    assert_eq!(
        partners,
        vec![b.to_string(), c.to_string()],
        "altes Werk (neueste zuerst): nur c ← b und b → c:\n{old_file}"
    );
    let new_file = store.idea_mirror_file(Some(meadow)).await.unwrap().unwrap().contents;
    assert!(!new_file.contains("- Verbindungen:"), "neues Werk ohne Verbindungszeile:\n{new_file}");
    // Wiederherstellen über die Werkgrenze bleibt abgelehnt (check_pair).
    let refused = store.idea_link_restore(ab.id).await;
    assert!(matches!(refused, Err(StoreError::Idea(_))), "Wiederherstellen über Werke abgelehnt: {refused:?}");

    // Die Lage von a im alten Werk ist hart entfernt (Grabstein mit der uuid des alten Werks), die von b bleibt.
    let left: Vec<IdeaId> =
        sqlx::query_scalar("SELECT idea_id FROM idea_map_pos ORDER BY idea_id").fetch_all(store.pool()).await.unwrap();
    assert_eq!(left, vec![b], "nur die Lage von b bleibt");
    let grave: Option<String> = sqlx::query_scalar("SELECT work_uuid FROM uuid_tombstone WHERE kind = 'idea_map_pos'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(grave.as_deref(), Some(orchard_uuid.as_str()), "Grabstein der Lage im alten Werk");

    // Zurück ins alte Werk: keine alte Lage taucht wieder auf, die Verbindung lässt sich wiederherstellen.
    let back = IdeaEdit { work: Some(Some(orchard)), ..IdeaEdit::default() };
    store.idea_update(a, &back).await.unwrap().expect("zurück");
    assert!(
        store.idea_map_positions(orchard).await.unwrap().iter().all(|p| p.idea != a),
        "keine alte Lage von a im alten Werk"
    );
    assert!(store.idea_link_get(ab.id).await.unwrap().deleted.is_some(), "Rückzug stellt nichts von selbst her");
    store.idea_link_restore(ab.id).await.expect("wieder im selben Werk: wiederherstellbar");

    // Ändern ohne Werkwechsel lässt Verbindungen und Lagen in Ruhe.
    store.idea_map_pos_set(orchard, a, 1.0, 2.0).await.unwrap();
    let note = IdeaEdit { note: Some("Nur ein Gedanke"), ..IdeaEdit::default() };
    store.idea_update(a, &note).await.unwrap().expect("geändert");
    assert_eq!(store.idea_links_of_idea(a, false).await.unwrap().len(), 1, "Verbindung a → b bleibt");
    assert_eq!(store.idea_map_positions(orchard).await.unwrap().len(), 2, "Lagen von a und b bleiben");
}
