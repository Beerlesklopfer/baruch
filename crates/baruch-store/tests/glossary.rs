// SPDX-License-Identifier: AGPL-3.0-or-later
//! Glossary, own indexes and index proposals in the store (Round 31a, contract §2.5, migration 0049; task
//! glossar-daten): creating with duplicate check through `term_key`, lookup through term and variants, sorting with
//! `sort_key` in Rust, proposals with origin, deciding (rejecting leaves a tombstone), changing and deleting, own
//! indexes (built-in identifiers locked, at most 13), index proposals through the fixed key and their acceptance as
//! a new revision of the human in one transaction or „veraltet“ without a text change; data for
//! `Store::back_matter` and the mirror file `glossar.md`. Real SQLite file in the temp folder; all texts freely
//! invented.

use baruch_store::{
    GlossaryAction, GlossaryEdit, GlossaryStatus, IndexProposalStatus, NewIndexProposal, Store, StoreError, WorkId,
};
use baruch_typst::index::{IndexMark, Occurrence, RegisterKind};

const BOOK: &str = "#set document(title: \"Die Herberge am Pass\", author: \"Tilde Marr\")\n\n\
= Ankunft\n\nDer Wirt reicht dem Pilger Brot. Brot und Salz sind das Zeichen der Gastfreundschaft.\n\n\
= Abschied\n\nAm Morgen segnet der Wirt den Pilger.\n";

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(BOOK, "herberge.typ", "Tilde").await.unwrap().work;
    (dir, store, work)
}

fn edit(term: &str) -> GlossaryEdit {
    GlossaryEdit { term: term.into(), short: format!("Kurz: {term}"), ..GlossaryEdit::default() }
}

async fn tombstone_kind(store: &Store, uuid: &str) -> Option<(String, Option<String>)> {
    sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(uuid)
        .fetch_optional(store.pool())
        .await
        .unwrap()
}

async fn work_uuid(store: &Store, work: WorkId) -> String {
    sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap()
}

/// Fixed key of the first section.
async fn first_key(store: &Store, work: WorkId) -> i64 {
    store.load_work(work).await.unwrap().nodes[0].key
}

fn glossary_message(result: Result<impl std::fmt::Debug, StoreError>) -> String {
    match result {
        Err(StoreError::Glossary(message)) => message,
        other => panic!("StoreError::Glossary erwartet, nicht {other:?}"),
    }
}

#[tokio::test]
async fn terms_are_unique_by_key_found_by_variants_and_sorted_in_rust() {
    let (_dir, store, work) = fresh().await;
    let salz = GlossaryEdit {
        term: "Salz".into(),
        short: "Zeichen des Bundes".into(),
        long: "Salz bewahrt vor Fäulnis.\nIm Bund steht es für Dauer.".into(),
        variants: vec!["Salzes".into(), "salzig".into()],
        source: "frei erfunden".into(),
        bible_refs: vec!["Lev 2,13".into()],
    };
    let added = store.glossary_add(work, &salz, "Tilde").await.expect("Begriff anlegen");
    assert_eq!(added.action, GlossaryAction::Added, "Aktion für den Spiegel-Commit");
    let term = added.after.clone().expect("nachher");
    assert_eq!(term.status, GlossaryStatus::Accepted, "der Mensch legt angenommene Begriffe an");
    assert_eq!(term.origin, "Tilde", "Herkunft = der Mensch");
    assert_eq!(term.variants, ["Salzes", "salzig"], "Varianten in ihrer Reihenfolge");
    assert_eq!(term.bible_refs, ["Lev 2,13"], "Bibelstellen als Text");
    assert_eq!(term.uuid.len(), 36, "uuid über den Fülltrigger: {}", term.uuid);
    store.glossary_add(work, &edit("Brot"), "Tilde").await.unwrap();
    store.glossary_add(work, &edit("Altar"), "Tilde").await.unwrap();

    // Duplicate through term_key (upper/lower case, whitespace) refused, nothing created.
    let message = glossary_message(store.glossary_add(work, &edit("  SALZ "), "Tilde").await);
    assert!(message.contains("gibt es schon") && message.contains("Salz"), "Meldung nennt den Begriff: {message}");
    let all = store.glossary(work, None).await.unwrap();
    assert_eq!(
        all.iter().map(|t| t.term.as_str()).collect::<Vec<_>>(),
        ["Altar", "Brot", "Salz"],
        "sortiert nach sort_key, keine Doppel"
    );

    // Lookup: term and variants, independent of upper/lower case; nothing for unknown words.
    for word in ["salz", "SALZES", " salzig "] {
        let found = store.glossary_lookup(work, word).await.unwrap();
        assert_eq!(found.len(), 1, "„{word}“ findet Salz über Begriff oder Variante: {found:?}");
        assert_eq!(found[0].id, term.id, "derselbe Begriff");
    }
    assert!(store.glossary_lookup(work, "Pfeffer").await.unwrap().is_empty(), "Unbekanntes findet nichts");
    assert!(store.glossary_lookup(work, "   ").await.unwrap().is_empty(), "leeres Wort findet nichts");

    // Changing: a new key works, a key of another accepted term does not.
    let changed = store
        .glossary_update(work, term.id, &GlossaryEdit { term: "Meersalz".into(), ..salz.clone() }, "Tilde")
        .await
        .expect("ändern");
    assert_eq!(changed.action, GlossaryAction::Updated, "Aktion");
    assert_eq!(changed.before.as_ref().unwrap().term, "Salz", "vorher für den Spiegel");
    let after = changed.after.unwrap();
    assert_eq!((after.term.as_str(), after.updated_by.as_str()), ("Meersalz", "Tilde"), "nachher mit updated_by");
    assert!(after.updated.is_some(), "updated gesetzt");
    assert_eq!(after.uuid, term.uuid, "uuid bleibt beim Ändern");
    assert_eq!(store.glossary_lookup(work, "salz").await.unwrap().len(), 0, "alter Schlüssel findet nichts mehr");
    let message = glossary_message(store.glossary_update(work, term.id, &edit("brot"), "Tilde").await);
    assert!(message.contains("Brot"), "Doppel beim Ändern abgelehnt: {message}");

    // Deleting leaves a tombstone with work uuid.
    let deleted = store.glossary_delete(work, term.id, "Tilde").await.expect("löschen");
    assert_eq!(deleted.action, GlossaryAction::Deleted, "Aktion");
    assert_eq!(
        tombstone_kind(&store, &term.uuid).await,
        Some(("glossary_term".into(), Some(work_uuid(&store, work).await))),
        "Grabstein glossary_term"
    );
    assert!(
        matches!(store.glossary_term(work, term.id).await, Err(StoreError::UnknownGlossaryTerm { .. })),
        "gelöscht → unbekannt"
    );
    assert!(
        matches!(store.glossary(9999, None).await, Err(StoreError::UnknownWork(9999))),
        "unbekanntes Werk wird gemeldet, nicht als leeres Glossar"
    );
}

#[tokio::test]
async fn proposals_wait_for_the_human_and_rejecting_leaves_a_tombstone() {
    let (_dir, store, work) = fresh().await;
    store.glossary_add(work, &edit("Brot"), "Tilde").await.unwrap();
    let message = glossary_message(store.glossary_propose(work, &edit("brot"), "@lektor").await);
    assert!(message.contains("schon im Glossar"), "angenommener Begriff lässt sich nicht vorschlagen: {message}");

    let proposed = store.glossary_propose(work, &edit("Pilger"), "@lektor").await.expect("vorschlagen");
    assert_eq!(proposed.action, GlossaryAction::Proposed, "Aktion");
    let pilger = proposed.after.unwrap();
    assert_eq!((pilger.status, pilger.origin.as_str()), (GlossaryStatus::Proposed, "@lektor"), "Status und Herkunft");
    let message = glossary_message(store.glossary_propose(work, &edit("PILGER"), "@mcp:probe").await);
    assert!(message.contains("schon vorgeschlagen"), "kein zweiter gleicher Vorschlag: {message}");
    assert!(store.glossary_lookup(work, "Pilger").await.unwrap().is_empty(), "Nachschlagen findet nur angenommene");
    assert!(
        store.back_matter(work).await.unwrap().glossary.iter().all(|e| e.term != "Pilger"),
        "Vorschläge kommen nicht ins Buchende"
    );
    assert_eq!(store.glossary(work, Some(GlossaryStatus::Proposed)).await.unwrap().len(), 1, "Filter proposed");
    assert_eq!(store.glossary(work, Some(GlossaryStatus::Accepted)).await.unwrap().len(), 1, "Filter accepted");

    let accepted = store.glossary_decide(work, pilger.id, true, "Tilde").await.expect("übernehmen");
    assert_eq!(accepted.action, GlossaryAction::Accepted, "Aktion");
    let after = accepted.after.unwrap();
    assert_eq!((after.status, after.origin.as_str()), (GlossaryStatus::Accepted, "@lektor"), "Herkunft bleibt");
    assert_eq!(after.updated_by, "Tilde", "der Mensch hat übernommen");
    let message = glossary_message(store.glossary_decide(work, pilger.id, false, "Tilde").await);
    assert!(message.contains("kein Vorschlag"), "nur Vorschläge lassen sich entscheiden: {message}");

    let wirt = store.glossary_propose(work, &edit("Wirt"), "@lektor").await.unwrap().after.unwrap();
    let rejected = store.glossary_decide(work, wirt.id, false, "Tilde").await.expect("verwerfen");
    assert_eq!((rejected.action, rejected.after.is_none()), (GlossaryAction::Rejected, true), "verworfen = gelöscht");
    assert_eq!(
        tombstone_kind(&store, &wirt.uuid).await.map(|(kind, _)| kind).as_deref(),
        Some("glossary_term"),
        "Verwerfen hinterlässt einen Grabstein (Vertrag §2.5)"
    );
}

#[tokio::test]
async fn custom_registers_are_limited_and_builtin_ones_are_locked() {
    let (_dir, store, work) = fresh().await;
    for slug in ["stichwort", "personen", "bibel"] {
        let message = glossary_message(store.index_register_add(work, slug, "Eigenes").await);
        assert!(message.contains("eingebautes Register"), "{slug} gesperrt: {message}");
        assert!(store.index_register_delete(work, slug).await.is_err(), "{slug} nicht löschbar");
        assert!(store.index_register_rename(work, slug, "X").await.is_err(), "{slug} nicht umbenennbar");
    }
    for bad in ["Orte", "1orte", "orte_x", "", &"o".repeat(33)] {
        let message = glossary_message(store.index_register_add(work, bad, "Orte").await);
        assert!(message.contains("Kennung"), "ungültige Kennung „{bad}“: {message}");
    }
    assert!(store.index_register_add(work, "orte", "  ").await.is_err(), "Titel nicht leer");
    assert!(store.index_register_add(work, "orte", &"T".repeat(81)).await.is_err(), "Titel höchstens 80 Zeichen");

    let orte = store.index_register_add(work, "orte", "Ortsregister").await.expect("anlegen");
    assert_eq!((orte.slug.as_str(), orte.position), ("orte", 0), "erstes eigenes Register");
    assert!(store.index_register_add(work, "orte", "Orte").await.is_err(), "Kennung je Werk eindeutig");
    let renamed = store.index_register_rename(work, "orte", "Orte und Wege").await.unwrap();
    assert_eq!((renamed.title.as_str(), renamed.uuid.as_str()), ("Orte und Wege", orte.uuid.as_str()), "uuid bleibt");
    for n in 1..13 {
        store.index_register_add(work, &format!("r{n}"), &format!("Register {n}")).await.unwrap();
    }
    let message = glossary_message(store.index_register_add(work, "r13", "zu viel").await);
    assert!(message.contains("Höchstens 13"), "höchstens 13 eigene Register: {message}");
    let back = store.back_matter(work).await.unwrap();
    assert_eq!(back.registers.len(), 3 + 13, "Buchende kennt eingebaute und eigene Register");
    assert_eq!(back.registers[3].slug, "orte", "eigene in ihrer Reihenfolge hinter den eingebauten");
    assert_eq!(back.registers[3].kind, RegisterKind::Custom, "Art Custom");

    let deleted = store.index_register_delete(work, "orte").await.expect("löschen");
    assert_eq!(
        tombstone_kind(&store, &deleted.uuid).await.map(|(kind, _)| kind).as_deref(),
        Some("index_register"),
        "Grabstein index_register"
    );
    assert_eq!(store.index_registers(work).await.unwrap().len(), 12, "eins weniger");
}

#[tokio::test]
async fn index_proposals_check_word_register_and_key() {
    let (_dir, store, work) = fresh().await;
    let key = first_key(&store, work).await;
    let proposal = |word: &str, register: &str| NewIndexProposal {
        key,
        word: word.into(),
        occurrence: Occurrence::First,
        mark: IndexMark::new(register, word),
        reason: "Leitwort des Kapitels".into(),
        origin: "@lektor".into(),
    };
    let message = glossary_message(store.index_propose(work, &proposal("Pfeffer", "stichwort")).await);
    assert!(message.contains("Pfeffer") && message.contains("steht nicht"), "Wort fehlt im Text: {message}");
    let message = glossary_message(store.index_propose(work, &proposal("Brot", "orte")).await);
    assert!(message.contains("orte"), "unbekanntes Register: {message}");
    let message = glossary_message(store.index_propose(work, &proposal("Brot", "Orte")).await);
    assert!(message.contains("keine gültige Kennung"), "ungültige Marke über IndexMark::validate: {message}");
    let gone = NewIndexProposal { key: 999_999, ..proposal("Brot", "stichwort") };
    assert!(
        matches!(store.index_propose(work, &gone).await, Err(StoreError::GoneNode { .. })),
        "fremder bzw. gelöschter Schlüssel → GoneNode"
    );

    let open = store.index_propose(work, &proposal("Brot", "stichwort")).await.expect("vorschlagen");
    assert_eq!((open.status, open.key, open.based_on), (IndexProposalStatus::Open, key, 1), "offen, auf Fassung 1");
    assert_eq!(open.origin, "@lektor", "Herkunft");
    let other = store.index_propose(work, &proposal("Salz", "stichwort")).await.unwrap();
    let rejected = store.index_proposal_reject(work, other.id, "Tilde").await.expect("verwerfen");
    assert_eq!(rejected.status, IndexProposalStatus::Rejected, "verworfen");
    assert_eq!(rejected.decided_by.as_deref(), Some("Tilde"), "wer entschieden hat");
    assert!(store.index_proposal_reject(work, other.id, "Tilde").await.is_err(), "schon entschieden");
    assert_eq!(
        store.index_proposals(work, Some(IndexProposalStatus::Open)).await.unwrap().len(),
        1,
        "nur der offene Vorschlag"
    );
    assert_eq!(store.index_proposals(work, None).await.unwrap().len(), 2, "alle");
    assert!(
        matches!(store.index_proposal_accept(work, 4242, "Tilde").await, Err(StoreError::UnknownIndexProposal { .. })),
        "unbekannter Vorschlag"
    );
}

#[tokio::test]
async fn accepting_an_index_proposal_is_a_new_human_revision_or_stale_without_change() {
    let (_dir, store, work) = fresh().await;
    let key = first_key(&store, work).await;
    let new = |word: &str| NewIndexProposal {
        key,
        word: word.into(),
        occurrence: Occurrence::All,
        mark: IndexMark::new("stichwort", word),
        reason: String::new(),
        origin: "@mcp:probe".into(),
    };
    let brot = store.index_propose(work, &new("Brot")).await.unwrap();
    let salz = store.index_propose(work, &new("Salz")).await.unwrap();

    let accepted = store.index_proposal_accept(work, brot.id, "Tilde").await.expect("übernehmen");
    assert_eq!(accepted.proposal.status, IndexProposalStatus::Accepted, "Status übernommen");
    assert_eq!(accepted.marks, 2, "„all“ markiert beide Fundstellen von „Brot“");
    assert_eq!(accepted.change.author, baruch_core::Author::Human { name: "Tilde".into() }, "Fassung des Menschen");
    let node = store.load_work(work).await.unwrap().nodes[0].clone();
    let current = node.revision(node.current_number().unwrap()).unwrap();
    assert_eq!(current.number, accepted.change.rev, "neue Fassung ist aktuell");
    assert!(current.body.contains("<baruch-idx>"), "Marke steht im Text: {}", current.body);

    // The same proposal once more: the marks already stand there – no second, equal mark, no new revision, the
    // proposal becomes outdated (finding after R31: duplicate marks on acceptance for a marked word).
    let again = store.index_propose(work, &new("Brot")).await.unwrap();
    let revisions = store.load_work(work).await.unwrap().nodes[0].revisions.len();
    let message = glossary_message(store.index_proposal_accept(work, again.id, "Tilde").await);
    assert!(message.contains("schon diese Registermarke"), "ehrlicher Hinweis: {message}");
    let node = store.load_work(work).await.unwrap().nodes[0].clone();
    assert_eq!(node.revisions.len(), revisions, "keine neue Fassung");
    assert_eq!(
        baruch_typst::index::find_marks(&node.revision(node.current_number().unwrap()).unwrap().body).len(),
        2,
        "weiter je Fundstelle eine Marke"
    );
    let stale = store.index_proposals(work, Some(IndexProposalStatus::Stale)).await.unwrap();
    assert_eq!(stale.iter().map(|p| p.id).collect::<Vec<_>>(), [again.id], "Status stale gespeichert");

    // The human removes „Salz“; the acceptance becomes outdated, the text stays.
    let without = current.body.replace("Salz", "Öl");
    let seq = store.node_seq(work, key).await.unwrap();
    store.save_human(work, seq, &without, "Tilde").await.unwrap().expect("gespeichert");
    let revisions = store.load_work(work).await.unwrap().nodes[0].revisions.len();
    let message = glossary_message(store.index_proposal_accept(work, salz.id, "Tilde").await);
    assert!(message.contains("veraltet"), "Hinweis auf veralteten Vorschlag: {message}");
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].revisions.len(), revisions, "keine neue Fassung");
    let stale = store.index_proposals(work, Some(IndexProposalStatus::Stale)).await.unwrap();
    let mut ids: Vec<_> = stale.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    let mut expected = vec![again.id, salz.id];
    expected.sort_unstable();
    assert_eq!(ids, expected, "Status stale gespeichert (beide)");
}

#[tokio::test]
async fn back_matter_and_mirror_carry_the_accepted_glossary() {
    let (_dir, store, work) = fresh().await;
    let files = store.mirror_snapshot(work).await.unwrap();
    assert!(files.iter().all(|f| f.path != "glossar.md"), "ohne Glossar keine Datei (Spiegel wie vor Runde 31)");

    let salz = GlossaryEdit {
        term: "Salz".into(),
        short: "Zeichen des Bundes".into(),
        variants: vec!["Salzes".into()],
        bible_refs: vec!["Lev 2,13".into()],
        source: "frei erfunden".into(),
        ..GlossaryEdit::default()
    };
    store.glossary_add(work, &salz, "Tilde").await.unwrap();
    store.glossary_add(work, &edit("Brot"), "Tilde").await.unwrap();
    store.glossary_propose(work, &edit("Herberge"), "@lektor").await.unwrap();
    store.index_register_add(work, "orte", "Ortsregister").await.unwrap();

    let back = store.back_matter(work).await.unwrap();
    assert_eq!(
        back.glossary.iter().map(|e| e.term.as_str()).collect::<Vec<_>>(),
        ["Brot", "Salz"],
        "nur angenommene Begriffe, sortiert"
    );
    let entry = &back.glossary[1];
    assert_eq!((entry.variants.clone(), entry.bible_refs.clone()), (vec!["Salzes".into()], vec!["Lev 2,13".into()]));
    assert!(back.registers.iter().any(|r| r.slug == "orte"), "eigenes Register im Buchende");

    let files = store.mirror_snapshot(work).await.unwrap();
    let md = &files.iter().find(|f| f.path == "glossar.md").expect("glossar.md im Spiegel").contents;
    for part in [
        "# Glossar – Die Herberge am Pass",
        "## Brot",
        "## Salz",
        "Varianten: Salzes",
        "Zeichen des Bundes",
        "Bibelstellen: Lev 2,13",
        "Quelle: frei erfunden",
        "### Herberge (vorgeschlagen von @lektor)",
        "- `orte`: Ortsregister",
    ] {
        assert!(md.contains(part), "glossar.md enthält „{part}“: {md}");
    }
    assert!(md.find("## Brot").unwrap() < md.find("## Salz").unwrap(), "alphabetisch");

    // Deleting the work: terms and indexes go along, tombstones with work uuid.
    let uuid = work_uuid(&store, work).await;
    let title = store.load_work(work).await.unwrap().title;
    store.delete_work(work, &title).await.unwrap();
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT kind FROM uuid_tombstone WHERE work_uuid = ? AND kind IN ('glossary_term','index_register') \
         ORDER BY kind",
    )
    .bind(&uuid)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(kinds, ["glossary_term", "index_register"], "Grabsteine der Kaskade kennen die Werk-uuid");
}
