// SPDX-License-Identifier: AGPL-3.0-or-later
//! Titelregeln der Gliederung (Vertrag Runde 7, 4b): leer, nur Leerraum, Zeilenumbruch und über
//! 200 Zeichen werden auf jedem Weg abgelehnt (Umbenennen, Anlegen, Speichern im Editor samt
//! Teilen, Prüfen, Speichern und Ändern der aktuellen Fassung, Vorschläge, Umschalten), und dann
//! ändert sich nichts. Vorhandene leere Titel bleiben lesbar und lassen sich umbenennen; Import und
//! Export bleiben byte-gleich. Echte SQLite-Datei im Temp-Ordner, Testtexte frei erfunden.

use baruch_store::{MAX_TITLE_CHARS, Place, Restructure, RevisionEdit, Store, StoreError, WorkId, check_title};

const HUMAN: &str = "Anna";

/// Eigenes kleines Buch; „Beete“ ohne Titel (`== `), wie es ein früherer Fehler hinterlassen hat.
const LAUBE: &str = "\
#set document(title: \"Anna's Laube\", author: \"Test\")

= Die Laube

Wein rankt am Gitter.

== Bänke

Zwei Bänke aus Lärche.

== \n\nOhne Titel geblieben.

= Der Teich

Seerosen.
";

async fn laube() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(LAUBE, "laube.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

/// Alles, was ein Fehlversuch nicht ändern darf: Export, Titel und Zahl der Fassungen.
async fn snapshot(store: &Store, work: WorkId) -> (String, Vec<String>, i64) {
    let export = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let titles = store.load_work(work).await.unwrap().nodes.into_iter().map(|n| n.title).collect();
    let revisions: i64 = sqlx::query_scalar("SELECT count(*) FROM revision").fetch_one(store.pool()).await.unwrap();
    (export, titles, revisions)
}

fn title_error(result: Result<impl std::fmt::Debug, StoreError>) -> String {
    match result {
        Err(StoreError::Title(message)) => message,
        other => panic!("StoreError::Title erwartet, nicht {other:?}"),
    }
}

const EMPTY: &str = "Ein Gliederungspunkt braucht einen Titel.";
const NEWLINE: &str = "Der Titel darf keinen Zeilenumbruch enthalten.";

fn too_long() -> String {
    "Laub".repeat(51) // 204 Zeichen
}

#[test]
fn check_title_trims_and_refuses_empty_multiline_and_overlong_titles() {
    assert_eq!(MAX_TITLE_CHARS, 200, "Grenze laut Vertrag");
    assert_eq!(check_title("  Anna's Laube \t").unwrap(), "Anna's Laube", "Rand-Leerraum entfällt");
    assert_eq!(title_error(check_title("")), EMPTY, "leer");
    assert_eq!(title_error(check_title(" \t\u{3000}\u{a0}")), EMPTY, "nur Leerraum (auch Unicode) ist leer");
    for bad in ["Laube\nBänke", "Laube\rBänke", "Laube\u{2028}Bänke", "Laube\u{85}Bänke", "Laube\x0bBänke"] {
        assert_eq!(title_error(check_title(bad)), NEWLINE, "{bad:?}: Zeilenende im Sinn von Typst");
    }
    let limit = "ä".repeat(200);
    assert_eq!(check_title(&limit).unwrap(), limit, "200 Zeichen (nicht Bytes) gehen noch");
    assert_eq!(
        title_error(check_title(&"ä".repeat(201))),
        "Der Titel ist zu lang (höchstens 200 Zeichen, hier 201).",
        "201 Zeichen sind zu viel, die Meldung nennt die Zahl"
    );
}

#[tokio::test]
async fn renaming_and_adding_refuse_bad_titles_and_change_nothing() {
    let (_dir, store, work) = laube().await;
    let before = snapshot(&store, work).await;
    for (bad, message) in [("   ", EMPTY.to_owned()), ("Bänke\nund Tisch", NEWLINE.to_owned())] {
        assert_eq!(title_error(store.outline_rename(work, 1, bad, HUMAN).await), message, "Umbenennen {bad:?}");
        assert_eq!(title_error(store.outline_add(work, 0, Place::Into, bad, HUMAN).await), message, "Anlegen {bad:?}");
    }
    let long = too_long();
    assert!(title_error(store.outline_rename(work, 1, &long, HUMAN).await).contains("hier 204"), "zu lang");
    assert!(
        title_error(store.outline_add(work, 0, Place::After, &long, HUMAN).await).contains("hier 204"),
        "zu langer neuer Titel"
    );
    assert_eq!(snapshot(&store, work).await, before, "nach jeder Ablehnung ist alles wie vorher");
}

#[tokio::test]
async fn saving_in_the_editor_checks_the_first_and_every_split_title() {
    let (_dir, store, work) = laube().await;
    let before = snapshot(&store, work).await;
    let split = Restructure { split: true, merge: false };
    let empty_first = "== \n\nZwei Bänke aus Lärche.\n\n";
    assert_eq!(title_error(store.outline_save(work, 1, empty_first, None, split, HUMAN).await), EMPTY, "erster Titel");
    let long_split = format!("== Bänke\n\nZwei Bänke.\n\n== {}\n\nNoch ein Absatz.\n\n", too_long());
    let message = title_error(store.outline_save(work, 1, &long_split, None, split, HUMAN).await);
    assert!(message.starts_with("Der Titel ist zu lang"), "geteilter Titel geprüft: {message}");
    let blank_split = "== Bänke\n\nZwei Bänke.\n\n==   \n\nRest.\n\n";
    assert_eq!(title_error(store.outline_save(work, 1, blank_split, None, split, HUMAN).await), EMPTY, "Teilen");
    assert_eq!(snapshot(&store, work).await, before, "nichts gespeichert, nichts geteilt");

    let check = store.outline_check(work, 1, &long_split).await.unwrap();
    assert!(!check.valid, "outline.check meldet den zu langen Titel als ungültig: {check:?}");
    assert!(check.message.as_deref().unwrap_or("").starts_with("Der Titel ist zu lang"), "mit Meldung: {check:?}");
    let check = store.outline_check(work, 1, empty_first).await.unwrap();
    assert_eq!((check.valid, check.message.as_deref()), (false, Some(EMPTY)), "leerer Titel im Check: {check:?}");
    let fine = store.outline_check(work, 1, "== Bänke aus Lärche\n\nText.\n").await.unwrap();
    assert!(fine.valid, "ein gültiger Titel bleibt gültig: {fine:?}");
}

#[tokio::test]
async fn saving_and_editing_the_current_version_and_proposals_check_the_title() {
    let (_dir, store, work) = laube().await;
    let before = snapshot(&store, work).await;
    let empty = "== \n\nZwei Bänke aus Lärche.\n";
    assert_eq!(title_error(store.save_human(work, 1, empty, HUMAN).await), EMPTY, "revision.save (aktuelle)");
    assert_eq!(title_error(store.save_human_on(work, 1, empty, HUMAN, 1).await), EMPTY, "revision.save mit Basis");
    let edit = RevisionEdit { body: Some(empty), note: None };
    assert_eq!(title_error(store.update_revision(work, 1, 1, &edit, HUMAN).await), EMPTY, "Fassung ändern (aktuell)");
    let long = format!("== {}\n\nText.\n", too_long());
    let message = title_error(store.add_proposal(work, 1, &long, "@lektor", "Testmodell", "").await);
    assert!(message.starts_with("Der Titel ist zu lang"), "Vorschlag (MCP propose_revision): {message}");
    assert_eq!(
        title_error(store.add_proposal(work, 1, empty, "@lektor", "Testmodell", "").await),
        EMPTY,
        "Vorschlag mit leerem Titel"
    );
    assert_eq!(snapshot(&store, work).await, before, "keine Fassung angelegt oder geändert");
}

#[tokio::test]
async fn switching_to_a_version_with_a_bad_title_is_refused_and_changes_nothing() {
    let (_dir, store, work) = laube().await;
    store.save_human(work, 1, "== Bänke\n\nDrei Bänke aus Lärche.\n", HUMAN).await.unwrap().expect("Fassung 2");
    // Eine alte Fassung zu ändern prüft nichts (nur die aktuelle bestimmt die Gliederung) …
    let edit = RevisionEdit { body: Some("==   \n\nZwei Bänke.\n"), note: None };
    store.update_revision(work, 1, 1, &edit, HUMAN).await.unwrap().expect("Fassung 1 geändert");
    let before = snapshot(&store, work).await;
    // … aber Umschalten auf sie ließe die Gliederung einem leeren Titel folgen.
    assert_eq!(title_error(store.set_current(work, 1, 1).await), EMPTY, "Umschalten (gleiche Ebene)");
    assert_eq!(snapshot(&store, work).await, before, "Umschalten abgelehnt, nichts geändert");
    // Ein Zeilenende im Sinn von Typst (U+2028) mitten in der Überschriftszeile, dazu eine andere Ebene.
    let edit = RevisionEdit { body: Some("= Bänke\u{2028}im Schatten\n\nText.\n"), note: None };
    let proposal = store.add_proposal(work, 1, "== Bänke im Schatten\n\nText.\n", "@lektor", "Testmodell", "").await;
    let proposal = proposal.expect("gültiger Vorschlag");
    store.update_revision(work, 1, proposal, &edit, HUMAN).await.unwrap().expect("Vorschlag vom Menschen geändert");
    let before_accept = snapshot(&store, work).await;
    assert_eq!(title_error(store.accept(work, 1, proposal).await), NEWLINE, "Übernehmen (andere Ebene)");
    assert_eq!(snapshot(&store, work).await, before_accept, "Übernehmen abgelehnt, nichts geändert");
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(loaded.nodes[1].current_number(), Some(2), "Fassung 2 bleibt aktuell");
    assert_eq!(before.1, before_accept.1, "Titel unverändert");
}

#[tokio::test]
async fn an_existing_empty_title_stays_readable_and_can_be_renamed() {
    let (_dir, store, work) = laube().await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        LAUBE,
        "Import und Export byte-gleich, auch mit `== `"
    );
    let loaded = store.load_work(work).await.unwrap();
    let titles: Vec<&str> = loaded.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["Die Laube", "Bänke", "", "Der Teich"], "der leere Titel bleibt, nichts entfernt");
    assert_eq!(store.load_work(work).await.unwrap().nodes.len(), 4, "zweites Laden ändert nichts");

    let done = store.outline_rename(work, 2, "Hecke", HUMAN).await.expect("Umbenennen auf einen gültigen Titel");
    assert_eq!(done.detail, "„“ → „Hecke“ umbenannt", "Betreff nennt den alten (leeren) Titel");
    let loaded = store.load_work(work).await.unwrap();
    let node = &loaded.nodes[2];
    assert_eq!(node.title, "Hecke", "neuer Titel");
    let current = &node.revisions[node.current_index().unwrap()];
    assert_eq!(current.body, "== Hecke\n\nOhne Titel geblieben.\n\n", "nur die Überschriftszeile ändert sich");
}

/// Wie LAUBE, dazu „Pflaster“ mit einem 201 Zeichen langen Titel (Import prüft nicht).
fn laube_with_long_title() -> (String, String) {
    let long_title = "Stein".repeat(40) + "e";
    (format!("{LAUBE}\n== {long_title}\n\nPflastersteine.\n"), long_title)
}

#[tokio::test]
async fn an_unchanged_bad_title_does_not_block_saving_but_a_changed_one_is_still_checked() {
    // Runde 8 (Gutachten-Rest): Wer in einem importierten Abschnitt mit leerem oder überlangem Titel nur
    // den Text ändert, soll speichern können; nur neue oder geänderte Titel werden geprüft.
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let (source, long_title) = laube_with_long_title();
    let work = store.import_typst(&source, "laube-lang.typ", HUMAN).await.unwrap().work;
    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(long_title.chars().count(), 201, "Testtitel hat 201 Zeichen");
    assert_eq!(loaded.nodes[2].title, "", "Punkt 2 hat den leeren Titel");
    assert_eq!(loaded.nodes[4].title, long_title, "Punkt 4 den überlangen");

    // Editor (outline.save), leerer Titel unverändert.
    let saved = store
        .outline_save(work, 2, "== \n\nNeu bepflanzt.\n", None, Restructure::default(), HUMAN)
        .await
        .expect("unveränderter leerer Titel lässt sich speichern");
    assert!(saved.change.is_some(), "neue Fassung angelegt");
    assert!(store.outline_check(work, 2, "== \n\nNoch einmal.\n").await.unwrap().valid, "outline.check ebenso");
    // revision.save (save_human) mit unverändertem 201-Zeichen-Titel.
    let body = format!("== {long_title}\n\nNeue Steine.\n");
    store.save_human(work, 4, &body, HUMAN).await.expect("save_human mit unverändertem langem Titel");
    // Vorschlag mit unverändertem Titel.
    let proposal = format!("== {long_title}\n\nVorschlag: Kies statt Steine.\n");
    store.add_proposal(work, 4, &proposal, "@lektor", "Testmodell", "").await.expect("Vorschlag geht");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(after.nodes[2].title, "", "Titel bleibt leer (nicht umbenannt)");
    assert_eq!(after.nodes[4].title, long_title, "langer Titel bleibt");

    // Geänderte ungültige Titel bleiben abgelehnt.
    let changed = format!("== {long_title}x\n\nText.\n");
    assert!(
        title_error(store.save_human(work, 4, &changed, HUMAN).await).contains("hier 202"),
        "ein geänderter überlanger Titel wird geprüft"
    );
    let message = title_error(store.add_proposal(work, 4, &changed, "@lektor", "Testmodell", "").await);
    assert!(message.starts_with("Der Titel ist zu lang"), "auch im Vorschlag: {message}");
    assert_eq!(
        title_error(store.outline_save(work, 1, "== \n\nBänke weg.\n", None, Restructure::default(), HUMAN).await),
        EMPTY,
        "„Bänke“ → leer ist eine Änderung und wird abgelehnt"
    );
    let split = Restructure { split: true, merge: false };
    let new_empty = "== \n\nText.\n\n==   \n\nNeu.\n";
    assert_eq!(
        title_error(store.outline_save(work, 2, new_empty, None, split, HUMAN).await),
        EMPTY,
        "ein neuer (geteilter) leerer Titel wird geprüft, auch wenn der erste unverändert leer ist"
    );
}
