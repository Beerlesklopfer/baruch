// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ideas as a tree in the store (Round 34 „Mindmap wie XMind“, contract §0/§3; addendum of the user of 2026-11-25
//! about the cloud) on a real SQLite file in the temp folder: title instead of keyword, subnodes, order,
//! re-parenting with the refusals (other work, cycle, depth, deleted), cloud and main topic, deleting and restoring
//! including subnodes, tree import in one transaction, full text through the title, mirror. Test texts freely
//! invented.

use baruch_store::{
    Idea, IdeaEdit, IdeaFilter, IdeaId, IdeaMove, IdeaOrigin, IdeaScope, IdeaSeed, IdeaSeedLink, IdeaSpot, LinkKind,
    MAX_IDEA_DEPTH, MAX_IDEA_IMPORT_NODES, NewIdea, NewIdeaLink, Store, StoreError, WorkId,
};

/// A second, freely invented work (the first is the sample work of the empty store).
const SECOND: &str =
    "#set document(title: \"Die Mühle am Bach\")\n\n= Rad\n\nDas Rad dreht sich.\n\n= Stein\n\nDer Stein mahlt.\n";

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

fn human() -> IdeaOrigin {
    IdeaOrigin::Human { name: "Autor".into() }
}

/// Fixed key of the section at position `seq`.
async fn key(store: &Store, work: WorkId, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

/// Create an idea with only a title at a place.
async fn titled(store: &Store, work: WorkId, title: &str, spot: IdeaSpot<'_>) -> Idea {
    let new = NewIdea {
        work: Some(work),
        key: None,
        words: &[],
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    store.idea_create_at(&new, &IdeaSpot { title, ..spot }).await.unwrap_or_else(|e| panic!("{title}: {e}"))
}

async fn child(store: &Store, work: WorkId, title: &str, parent: IdeaId) -> Idea {
    titled(store, work, title, IdeaSpot { parent: Some(parent), ..IdeaSpot::default() }).await
}

/// The subnodes that are not deleted, in their order (`position`, then `id`), as titles.
async fn children(store: &Store, work: WorkId, parent: Option<IdeaId>) -> Vec<String> {
    let mut rows: Vec<Idea> =
        store.idea_tree_rows(work).await.unwrap().into_iter().filter(|i| i.parent == parent).collect();
    rows.sort_by_key(|i| (i.position, i.id));
    rows.into_iter().map(|i| i.title).collect()
}

fn refused(result: Result<Option<Idea>, StoreError>, part: &str, why: &str) {
    match result {
        Err(StoreError::Idea(message)) => assert!(message.contains(part), "{why}: Meldung nennt „{part}“: {message}"),
        other => panic!("{why}: Ablehnung erwartet, nicht {other:?}"),
    }
}

#[tokio::test]
async fn an_idea_needs_a_title_or_a_word_and_lands_in_the_cloud_without_a_place() {
    let (_dir, store, work) = fresh().await;
    // Contract §0: title or at least one keyword (before Round 34 the keyword was mandatory).
    let only_title = titled(&store, work, "  Der   Steg\nam See ", IdeaSpot::default()).await;
    assert_eq!(only_title.title, "Der Steg am See", "der Titel ist eine Zeile, Leerraum zusammengefasst");
    assert!(only_title.words.is_empty(), "ohne Schlagwort angelegt, weil ein Titel da ist");
    assert!(
        only_title.cloud && !only_title.main_topic && only_title.parent.is_none(),
        "ohne Elternknoten, Bezug und Hauptthema liegt der neue Einfall in der Wolke (Nachtrag 2026-11-25)"
    );
    let empty = NewIdea {
        work: Some(work),
        key: None,
        words: &[],
        note: "nur ein Gedanke",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let error = store.idea_create_at(&empty, &IdeaSpot::default()).await.unwrap_err();
    assert!(
        matches!(&error, StoreError::Idea(m) if m.contains("Titel oder mindestens ein Schlagwort")),
        "ohne Titel und Schlagwort abgelehnt, die Meldung nennt beides: {error}"
    );
    let long = "ä".repeat(201);
    let too_long = store.idea_create_at(&empty, &IdeaSpot { title: &long, ..IdeaSpot::default() }).await;
    assert!(
        matches!(too_long, Err(StoreError::Idea(m)) if m.contains("200")),
        "ein Titel über 200 Zeichen wird abgelehnt"
    );
    let exact = "ä".repeat(200);
    assert!(
        store.idea_create_at(&empty, &IdeaSpot { title: &exact, ..IdeaSpot::default() }).await.is_ok(),
        "200 Zeichen (nicht Bytes) gehen"
    );
    assert_eq!(store.idea_tree_rows(work).await.unwrap().len(), 2, "die abgelehnten Versuche haben nichts angelegt");

    // The old way stays: only keywords, no title – that lands in the cloud too.
    let list = vec!["Schilf".to_owned()];
    let classic = NewIdea {
        work: Some(work),
        key: None,
        words: &list,
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let classic = store.idea_create(&classic).await.unwrap();
    assert!(classic.cloud && classic.title.is_empty(), "idea_create ohne Platz: Wolke, kein Titel");

    // With a reference it hangs at the section, as main topic at the work – both not in the cloud.
    let k = key(&store, work, 1).await;
    let anchored = NewIdea {
        work: Some(work),
        key: Some(k),
        words: &list,
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let anchored = store.idea_create(&anchored).await.unwrap();
    assert!(!anchored.cloud && anchored.key == Some(k), "ein Einfall mit Bezug liegt nicht in der Wolke");
    let topic = titled(&store, work, "Ufer", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    assert!(topic.main_topic && !topic.cloud, "als freies Hauptthema am Werk, nicht in der Wolke");

    // Changing: the title can be set and emptied as long as a keyword stays.
    let renamed = store
        .idea_update(classic.id, &IdeaEdit { title: Some("Schilfgürtel"), ..IdeaEdit::default() })
        .await
        .unwrap()
        .expect("Umbenennen ist eine Änderung");
    assert_eq!(renamed.title, "Schilfgürtel", "idea_update setzt den Titel");
    let same = store.idea_update(classic.id, &IdeaEdit { title: Some(" Schilfgürtel "), ..IdeaEdit::default() }).await;
    assert!(same.unwrap().is_none(), "derselbe Titel (nach dem Säubern) ist keine Änderung");
    let no_words: Vec<String> = Vec::new();
    let bare = IdeaEdit { title: Some(""), words: Some(&no_words), ..IdeaEdit::default() };
    assert!(
        matches!(store.idea_update(classic.id, &bare).await, Err(StoreError::Idea(_))),
        "Titel und Schlagwörter zugleich leeren geht nicht"
    );
    let cleared =
        store.idea_update(classic.id, &IdeaEdit { title: Some(""), ..IdeaEdit::default() }).await.unwrap().unwrap();
    assert!(cleared.title.is_empty() && cleared.words == ["Schilf"], "Titel leer, das Schlagwort trägt den Einfall");
}

#[tokio::test]
async fn children_are_ordered_by_position_and_before_puts_one_in_front_of_a_sibling() {
    let (_dir, store, work) = fresh().await;
    let root = titled(&store, work, "Hafen", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let a = child(&store, work, "Kran", root.id).await;
    let b = child(&store, work, "Mole", root.id).await;
    assert_eq!((a.parent, b.parent), (Some(root.id), Some(root.id)), "beide sind Unterknoten des Hauptthemas");
    assert_eq!(children(&store, work, Some(root.id)).await, ["Kran", "Mole"], "neue Unterknoten kommen ans Ende");
    assert_eq!(
        (a.position, b.position),
        (0, 0),
        "solange nichts geordnet wurde, bleibt die Vorgabe 0 (Reihenfolge nach id; im Austausch ohne platz)"
    );
    assert!(!a.cloud && !a.main_topic, "ein Unterknoten ist weder Wolke noch Hauptthema");
    let c = titled(
        &store,
        work,
        "Leuchtfeuer",
        IdeaSpot { parent: Some(root.id), before: Some(b.id), ..IdeaSpot::default() },
    )
    .await;
    assert_eq!(children(&store, work, Some(root.id)).await, ["Kran", "Leuchtfeuer", "Mole"], "before ordnet davor ein");
    assert_eq!(c.position, 2, "die Geschwister sind lückenlos ab 1 nummeriert");

    // A "sibling" under another parent node is none.
    let other = titled(&store, work, "Werft", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let stranger = child(&store, work, "Helling", other.id).await;
    let new = NewIdea {
        work: Some(work),
        key: None,
        words: &[],
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let wrong = store
        .idea_create_at(
            &new,
            &IdeaSpot { title: "Poller", parent: Some(root.id), before: Some(stranger.id), main_topic: false },
        )
        .await;
    assert!(matches!(wrong, Err(StoreError::Idea(m)) if m.contains("kein Geschwister")), "fremdes before abgelehnt");
    assert_eq!(children(&store, work, Some(root.id)).await.len(), 3, "und nichts angelegt (eine Transaktion)");

    // Ordering without re-parenting: before a sibling or to the end; the same once more is no change.
    let moved = store.idea_move(b.id, &IdeaMove { before: Some(Some(a.id)), ..IdeaMove::default() }).await.unwrap();
    assert!(moved.is_some(), "Mole vor Kran ist eine Änderung");
    assert_eq!(children(&store, work, Some(root.id)).await, ["Mole", "Kran", "Leuchtfeuer"], "neu geordnet");
    let again = store.idea_move(b.id, &IdeaMove { before: Some(Some(a.id)), ..IdeaMove::default() }).await.unwrap();
    assert!(again.is_none(), "steht schon dort: nichts geändert, kein neuer Zeitstempel");
    let to_end = store.idea_move(b.id, &IdeaMove { before: Some(None), ..IdeaMove::default() }).await.unwrap();
    assert!(to_end.is_some(), "before: null hängt ans Ende");
    assert_eq!(children(&store, work, Some(root.id)).await, ["Kran", "Leuchtfeuer", "Mole"], "wieder am Ende");
    let updated_a = store.idea_get(a.id).await.unwrap().updated;
    assert_eq!(updated_a, a.updated, "die Geschwister behalten ihr updated: Reihenfolge ist kein Inhalt");
    // Only the reference of a subnode changes (the link to the section): it stays at its place.
    let k = key(&store, work, 1).await;
    let linked = store.idea_move(a.id, &IdeaMove { key: Some(Some(k)), ..IdeaMove::default() }).await.unwrap();
    assert_eq!(linked.map(|i| (i.key, i.parent)), Some((Some(k), Some(root.id))), "Bezug gesetzt, Elternknoten bleibt");
    assert_eq!(
        children(&store, work, Some(root.id)).await,
        ["Kran", "Leuchtfeuer", "Mole"],
        "ein neuer Bezug ordnet einen Unterknoten nicht um"
    );
}

#[tokio::test]
async fn moving_rehangs_and_refuses_other_works_circles_depth_and_deleted_ideas() {
    let (_dir, store, work) = fresh().await;
    let k = key(&store, work, 1).await;
    let hub = titled(&store, work, "Markt", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let stall = child(&store, work, "Stand", hub.id).await;
    let apple = child(&store, work, "Apfel", stall.id).await;
    let loose = titled(&store, work, "Regen", IdeaSpot::default()).await;

    // From the cloud onto a node: subnode.
    let hung = store
        .idea_move(loose.id, &IdeaMove { parent: Some(Some(stall.id)), ..IdeaMove::default() })
        .await
        .unwrap()
        .expect("Einhängen ist eine Änderung");
    assert!(hung.parent == Some(stall.id) && !hung.cloud, "aus der Wolke unter einen Knoten gehängt");
    assert_eq!(children(&store, work, Some(stall.id)).await, ["Apfel", "Regen"], "ans Ende der neuen Geschwister");

    // Onto a chapter: parent node detached, reference set.
    let at_chapter = store
        .idea_move(loose.id, &IdeaMove { parent: Some(None), key: Some(Some(k)), ..IdeaMove::default() })
        .await
        .unwrap()
        .unwrap();
    assert!(
        at_chapter.parent.is_none() && at_chapter.key == Some(k) && !at_chapter.main_topic && !at_chapter.cloud,
        "am Abschnitt: kein Elternknoten, Bezug gesetzt, weder Hauptthema noch Wolke"
    );
    // Onto the work (contract §7: both null): free main topic.
    let at_work = store
        .idea_move(loose.id, &IdeaMove { parent: Some(None), key: Some(None), ..IdeaMove::default() })
        .await
        .unwrap()
        .unwrap();
    assert!(at_work.main_topic && !at_work.cloud, "aufs Werk gezogen: freies Hauptthema");
    // Into the cloud, with subnodes: the subtree stays together.
    let cloud =
        store.idea_move(stall.id, &IdeaMove { cloud: Some(true), ..IdeaMove::default() }).await.unwrap().unwrap();
    assert!(cloud.cloud && cloud.parent.is_none() && cloud.key.is_none(), "in die Wolke gelöst");
    assert_eq!(
        store.idea_get(apple.id).await.unwrap().parent,
        Some(stall.id),
        "die Unterknoten bleiben am Einfall (Entscheidung: der Teilbaum wandert als Ganzes in die Wolke)"
    );
    let again = store.idea_move(stall.id, &IdeaMove { cloud: Some(true), ..IdeaMove::default() }).await.unwrap();
    assert!(again.is_none(), "liegt schon in der Wolke: nichts geändert");
    refused(
        store
            .idea_move(stall.id, &IdeaMove { cloud: Some(true), parent: Some(Some(hub.id)), ..IdeaMove::default() })
            .await,
        "Wolke",
        "Wolke und Elternknoten zugleich",
    );
    // From the cloud to main topic without a parent node.
    let topic =
        store.idea_move(stall.id, &IdeaMove { cloud: Some(false), ..IdeaMove::default() }).await.unwrap().unwrap();
    assert!(topic.main_topic && !topic.cloud, "cloud: false macht den Einfall ohne Platz zum Hauptthema");

    // Cycle: under itself or one of its own descendants.
    refused(
        store.idea_move(stall.id, &IdeaMove { parent: Some(Some(stall.id)), ..IdeaMove::default() }).await,
        "unter sich selbst",
        "eigener Elternknoten",
    );
    refused(
        store.idea_move(stall.id, &IdeaMove { parent: Some(Some(apple.id)), ..IdeaMove::default() }).await,
        "eigenen Unterknoten",
        "unter den eigenen Nachfahren",
    );
    assert!(store.idea_get(stall.id).await.unwrap().parent.is_none(), "nach der Ablehnung unverändert");

    // Other work.
    let other = store.import_typst(SECOND, "muehle.typ", "Autor").await.unwrap().work;
    assert_ne!(other, work, "Vorbedingung: ein zweites Werk");
    let foreign = titled(&store, other, "Fremd", IdeaSpot::default()).await;
    refused(
        store.idea_move(apple.id, &IdeaMove { parent: Some(Some(foreign.id)), ..IdeaMove::default() }).await,
        "anderen Werk",
        "Elternknoten in einem anderen Werk",
    );
    let new = NewIdea {
        work: Some(work),
        key: None,
        words: &[],
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let wrong =
        store.idea_create_at(&new, &IdeaSpot { title: "Quer", parent: Some(foreign.id), ..IdeaSpot::default() }).await;
    assert!(matches!(wrong, Err(StoreError::Idea(m)) if m.contains("anderen Werk")), "auch beim Anlegen abgelehnt");

    // Deleted: neither re-parent nor use as a parent node.
    store.idea_delete(loose.id).await.unwrap();
    refused(
        store.idea_move(loose.id, &IdeaMove { parent: Some(Some(hub.id)), ..IdeaMove::default() }).await,
        "gelöscht",
        "gelöschter Einfall",
    );
    refused(
        store.idea_move(apple.id, &IdeaMove { parent: Some(Some(loose.id)), ..IdeaMove::default() }).await,
        "gelöscht",
        "gelöschter Elternknoten",
    );

    // Depth: a chain of 32 works, the 33rd does not – neither when creating nor by re-parenting a subtree.
    let mut chain =
        vec![titled(&store, work, "Kette 1", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await.id];
    for n in 2..=MAX_IDEA_DEPTH {
        chain.push(child(&store, work, &format!("Kette {n}"), *chain.last().unwrap()).await.id);
    }
    assert_eq!(chain.len(), 32, "Vorbedingung: Kette der Tiefe 32");
    let too_deep = store
        .idea_create_at(&new, &IdeaSpot { title: "Kette 33", parent: chain.last().copied(), ..IdeaSpot::default() })
        .await;
    assert!(matches!(too_deep, Err(StoreError::Idea(m)) if m.contains("höchstens 32")), "der 33. wird abgelehnt");
    // „Stand“ has one subnode (height 2): under chain 30 it would be 32 (works), under chain 31 already 33.
    refused(
        store.idea_move(stall.id, &IdeaMove { parent: Some(Some(chain[30])), ..IdeaMove::default() }).await,
        "höchstens 32",
        "Teilbaum der Höhe 2 unter Tiefe 31",
    );
    let fits = store.idea_move(stall.id, &IdeaMove { parent: Some(Some(chain[29])), ..IdeaMove::default() }).await;
    assert!(fits.unwrap().is_some(), "unter Tiefe 30 passt der Teilbaum genau (30 + 2 = 32)");
}

#[tokio::test]
async fn deleting_takes_the_subtree_with_one_timestamp_and_restoring_brings_exactly_it_back() {
    let (_dir, store, work) = fresh().await;
    let root = titled(&store, work, "Garten", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let bed = child(&store, work, "Beet", root.id).await;
    let rose = child(&store, work, "Rose", bed.id).await;
    let weed = child(&store, work, "Quecke", bed.id).await;
    let shed = child(&store, work, "Schuppen", root.id).await;
    let link = store
        .idea_link_create(&NewIdeaLink {
            from: rose.id,
            to: shed.id,
            kind: LinkKind::LeadsTo,
            note: "Rankhilfe",
            origin: human(),
        })
        .await
        .unwrap()
        .0;
    store.idea_map_pos_set(work, rose.id, 10.0, 20.0).await.unwrap();

    // Deleted individually earlier: different timestamp, does not come back when the bed is restored.
    let early = store.idea_delete_tree(weed.id).await.unwrap();
    assert_eq!(early.ids, [weed.id], "ein Blatt löscht nur sich");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let gone = store.idea_delete_tree(bed.id).await.unwrap();
    assert_eq!(
        gone.ids,
        [bed.id, rose.id],
        "das Beet nimmt die nicht gelöschte Rose mit, die Quecke war schon gelöscht"
    );
    let stamp = gone.idea.deleted.clone().expect("gelöscht mit Zeitstempel");
    assert_eq!(store.idea_get(rose.id).await.unwrap().deleted.as_deref(), Some(stamp.as_str()), "derselbe Zeitstempel");
    assert_ne!(
        store.idea_get(weed.id).await.unwrap().deleted.as_deref(),
        Some(stamp.as_str()),
        "die Quecke trägt ihren älteren"
    );
    assert_eq!(children(&store, work, Some(root.id)).await, ["Schuppen"], "im Baum bleibt nur der Schuppen");
    let found =
        store.idea_search("Rose", &IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() }).await.unwrap();
    assert!(found.is_empty(), "mitgelöschte Unterknoten stehen nicht mehr im Volltextindex");
    let link_state: (i64, Option<String>) = sqlx::query_as("SELECT count(*), max(deleted) FROM idea_link WHERE id = ?")
        .bind(link.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(link_state, (1, None), "die Verbindung der Rose bleibt gespeichert und ungelöst");
    let positions: i64 = sqlx::query_scalar("SELECT count(*) FROM idea_map_pos WHERE idea_id = ?")
        .bind(rose.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(positions, 1, "die Lage der Rose bleibt gespeichert");
    assert!(matches!(store.idea_delete(bed.id).await, Err(StoreError::Idea(_))), "zweimal löschen geht nicht");

    let back = store.idea_restore_tree(bed.id).await.unwrap();
    assert_eq!(back.ids, [bed.id, rose.id], "genau die mit diesem Zeitstempel kommen zurück");
    assert!(!back.detached, "der Elternknoten ist da: nichts gelöst");
    assert_eq!(children(&store, work, Some(bed.id)).await, ["Rose"], "die Rose hängt wieder am Beet, die Quecke nicht");
    assert!(store.idea_get(weed.id).await.unwrap().deleted.is_some(), "die Quecke bleibt gelöscht");
    let found =
        store.idea_search("Rose", &IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() }).await.unwrap();
    assert_eq!(found.len(), 1, "wiederhergestellte Unterknoten stehen wieder im Volltextindex");

    // Parent node still deleted: the restored idea is detached and lies in the cloud.
    store.idea_delete_tree(root.id).await.unwrap();
    let alone = store.idea_restore_tree(bed.id).await.unwrap();
    assert!(alone.detached, "der Garten ist noch gelöscht: detached");
    assert!(alone.idea.parent.is_none() && alone.idea.cloud, "ohne Elternknoten und Bezug in der Wolke");
    assert_eq!(alone.ids, [bed.id, rose.id], "seine eigenen Unterknoten kommen mit");
    assert!(store.idea_get(root.id).await.unwrap().deleted.is_some(), "der Garten selbst bleibt gelöscht");
    assert!(
        matches!(store.idea_restore(bed.id).await, Err(StoreError::Idea(_))),
        "nicht gelöscht: kein Wiederherstellen"
    );
}

#[tokio::test]
async fn a_change_of_work_loosens_the_idea_and_leaves_its_children_behind() {
    let (_dir, store, work) = fresh().await;
    let other = store.import_typst(SECOND, "muehle.typ", "Autor").await.unwrap().work;
    let root = titled(&store, work, "Brücke", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let pier = child(&store, work, "Pfeiler", root.id).await;
    let stone = child(&store, work, "Quader", pier.id).await;
    let moved = store
        .idea_update(pier.id, &IdeaEdit { work: Some(Some(other)), ..IdeaEdit::default() })
        .await
        .unwrap()
        .expect("Werkwechsel ist eine Änderung");
    assert!(
        moved.work == Some(other) && moved.parent.is_none(),
        "im neuen Werk ohne Elternknoten (der gehört zum alten)"
    );
    assert!(moved.cloud, "und ohne Platz: in der Wolke des neuen Werks");
    let left = store.idea_get(stone.id).await.unwrap();
    assert!(
        left.work == Some(work) && left.parent.is_none(),
        "der Unterknoten bleibt im alten Werk und verliert den Elternknoten (kein Baum über zwei Werke)"
    );
}

fn seed(title: &str, children: Vec<IdeaSeed>) -> IdeaSeed {
    IdeaSeed { title: title.to_owned(), note: String::new(), words: Vec::new(), children }
}

#[tokio::test]
async fn a_tree_is_imported_in_one_transaction_or_not_at_all() {
    let (_dir, store, work) = fresh().await;
    let nodes = vec![
        IdeaSeed {
            title: "Küste".into(),
            note: "Salz in der Luft.".into(),
            words: vec!["Meer".into()],
            children: vec![seed("Düne", vec![seed("Strandhafer", vec![])]), seed("Watt", vec![])],
        },
        seed("", vec![]),
    ];
    let links = vec![
        IdeaSeedLink { from: vec![0, 0, 0], to: vec![0, 1], note: "hält den Sand".into() },
        IdeaSeedLink { from: vec![0], to: vec![0], note: "auf sich selbst".into() },
    ];
    let origin = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let (ids, linked) = store.idea_import_tree(work, None, &nodes, &links, &origin).await.unwrap();
    assert_eq!(ids.len(), 5, "fünf Knoten, in Tiefensuche");
    assert_eq!(linked, 1, "eine Verbindung; die eines Knotens mit sich selbst wird übergangen");
    let rows = store.idea_tree_rows(work).await.unwrap();
    let by_title = |title: &str| rows.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("kein {title}"));
    let coast = by_title("Küste");
    assert_eq!(ids[0], coast.id, "die Reihenfolge der Antwort beginnt mit dem ersten obersten Knoten");
    assert!(coast.main_topic && !coast.cloud, "ohne Elternknoten eingelesen: freie Hauptthemen, nicht Wolke");
    assert_eq!(
        (coast.note.as_str(), coast.words.clone()),
        ("Salz in der Luft.", vec!["Meer".to_owned()]),
        "Notiz und Schlagwörter"
    );
    assert_eq!(coast.origin, origin, "die übergebene Herkunft steht an jedem Einfall");
    assert_eq!(by_title("Strandhafer").parent, Some(by_title("Düne").id), "die Verschachtelung bleibt");
    assert_eq!(
        children(&store, work, Some(coast.id)).await,
        ["Düne", "Watt"],
        "Geschwister in der Reihenfolge der Eingabe"
    );
    assert!(
        by_title("(ohne Titel)").main_topic,
        "ein Knoten ohne Titel und Schlagwort bekommt den Titel „(ohne Titel)“"
    );
    let stored = store.idea_links_of_work(work, false).await.unwrap();
    assert_eq!(stored.len(), 1, "genau eine Verbindung gespeichert");
    assert_eq!(
        (stored[0].from, stored[0].to, stored[0].kind, stored[0].note.as_str()),
        (by_title("Strandhafer").id, by_title("Watt").id, LinkKind::LeadsTo, "hält den Sand"),
        "als leads_to mit der Beschriftung als Notiz"
    );

    // Under a node.
    let (below, _) =
        store.idea_import_tree(work, Some(coast.id), &[seed("Priel", vec![])], &[], &human()).await.unwrap();
    let priel = store.idea_get(below[0]).await.unwrap();
    assert!(priel.parent == Some(coast.id) && !priel.main_topic, "unter dem gewählten Knoten, nicht Hauptthema");
    assert_eq!(
        children(&store, work, Some(coast.id)).await,
        ["Düne", "Watt", "Priel"],
        "hinter den vorhandenen Geschwistern"
    );

    // Exceeded limits: nothing created.
    let count = store.idea_tree_rows(work).await.unwrap().len();
    let too_many: Vec<IdeaSeed> = (0..=MAX_IDEA_IMPORT_NODES).map(|n| seed(&format!("Knoten {n}"), vec![])).collect();
    let error = store.idea_import_tree(work, None, &too_many, &[], &human()).await.unwrap_err();
    assert!(matches!(&error, StoreError::Idea(m) if m.contains("höchstens 2000")), "2001 Knoten abgelehnt: {error}");
    let mut deep = seed("unten", vec![]);
    for n in 0..MAX_IDEA_DEPTH {
        deep = seed(&format!("Ebene {n}"), vec![deep]);
    }
    let error = store.idea_import_tree(work, None, &[deep], &[], &human()).await.unwrap_err();
    assert!(matches!(&error, StoreError::Idea(m) if m.contains("höchstens 32")), "33 Ebenen abgelehnt: {error}");
    let bad_link = [IdeaSeedLink { from: vec![0], to: vec![7, 7], note: String::new() }];
    let error = store.idea_import_tree(work, None, &[seed("Allein", vec![])], &bad_link, &human()).await.unwrap_err();
    assert!(matches!(&error, StoreError::Idea(m) if m.contains("nicht gibt")), "Pfad ohne Knoten abgelehnt: {error}");
    let long_title = [seed("gut", vec![seed(&"x".repeat(201), vec![])])];
    assert!(
        matches!(store.idea_import_tree(work, None, &long_title, &[], &human()).await, Err(StoreError::Idea(_))),
        "ein zu langer Titel tief im Baum lehnt den ganzen Baum ab"
    );
    assert!(
        matches!(store.idea_import_tree(work, None, &[], &[], &human()).await, Err(StoreError::Idea(_))),
        "ein leerer Baum wird abgelehnt"
    );
    assert_eq!(
        store.idea_tree_rows(work).await.unwrap().len(),
        count,
        "keine der Ablehnungen hat etwas angelegt – auch nicht den gültigen Anfang (eine Transaktion)"
    );
    assert!(
        matches!(
            store.idea_import_tree(999_999, None, &[seed("x", vec![])], &[], &human()).await,
            Err(StoreError::UnknownWork(_))
        ),
        "unbekanntes Werk"
    );
}

#[tokio::test]
async fn the_title_is_searchable_and_the_mirror_shows_title_and_tree() {
    let (_dir, store, work) = fresh().await;
    let root = titled(&store, work, "Glockenturm", IdeaSpot { main_topic: true, ..IdeaSpot::default() }).await;
    let bell = child(&store, work, "Bronzeglocke", root.id).await;
    let list = vec!["Seil".to_owned()];
    let loose = NewIdea {
        work: Some(work),
        key: None,
        words: &list,
        note: "",
        quote: "",
        source: "",
        bible: "",
        origin: human(),
    };
    let loose = store.idea_create(&loose).await.unwrap();
    let filter = IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() };
    let found = store.idea_search("Bronzeglocke", &filter).await.unwrap();
    assert_eq!(
        found.iter().map(|(i, _)| i.id).collect::<Vec<_>>(),
        [bell.id],
        "die Volltextsuche findet über den Titel"
    );
    store.idea_update(bell.id, &IdeaEdit { title: Some("Zinnglocke"), ..IdeaEdit::default() }).await.unwrap();
    assert!(store.idea_search("Bronzeglocke", &filter).await.unwrap().is_empty(), "der alte Titel ist aus dem Index");
    assert_eq!(store.idea_search("Zinnglocke", &filter).await.unwrap().len(), 1, "der neue steht darin");

    let mirror = store.idea_mirror_file(Some(work)).await.unwrap().expect("einfaelle.md gibt es mit Einfällen");
    let text = mirror.contents;
    let tree = format!(
        "\n## Baum\n\n- Einfall {}: Glockenturm (Hauptthema)\n  - Einfall {}: Zinnglocke\n- Einfall {}: Seil (Wolke)\n",
        root.id, bell.id, loose.id
    );
    assert!(text.contains(&tree), "der Spiegel zeigt den Baum mit Titeln, eingerückt, mit Platz:\n{text}");
    assert!(text.contains("- Titel: Zinnglocke\n"), "der Eintrag des Einfalls nennt den Titel:\n{text}");
    assert!(
        text.contains(&format!("- Unterknoten von: Einfall {}\n", root.id)),
        "der Eintrag nennt den Elternknoten:\n{text}"
    );
    assert!(text.contains("- Platz: Hauptthema am Werk\n"), "der Eintrag des Hauptthemas nennt seinen Platz:\n{text}");
}
