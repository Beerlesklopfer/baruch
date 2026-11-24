// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a (Härtung, Besitz web) auf echten SQLite-Dateien im Temp-Ordner, mit frei erfundenen Werken:
//! - „Ändern über den Baum“ (`update_revision`) legt auf der aktuellen bzw. einer fortgeschriebenen Fassung einen
//!   Schritt im Verlauf an und erhöht den Stand; ein veralteter Stand wird abgelehnt (`StoreError::Stale`).
//! - Befunde: offene Vorschläge in einer Abfrage, Sammelbefunde zählen nicht in „k offen“ und werden „veraltet“.
//! - `ExportMap`: Zeilen des Buchendes gehören nicht zum letzten Abschnitt.

use baruch_core::{Post, RevisionStatus};
use baruch_store::{
    AmendKind, ChangeKind, FindingFilter, FindingKind, FindingStatus, IdeaOrigin, NewFinding, NewReason, RevisionEdit,
    SaveMode, StatusChange, Store, StoreError, WorkId,
};

const BOOK: &str = "#set document(title: \"Die Glashütte\", author: \"Mara Feld\")\n\n\
= Ofen\n\nDer Ofen brannte seit dem Morgen. Die Bläser standen im Kreis und warteten, bis das Glas weich genug war, \
um es an der Pfeife zu drehen; draußen fiel der erste Schnee auf den Hof.\n\n\
= Sand\n\nDer Sand kam vom Fluss. Er wurde gewaschen, gesiebt und in Säcken unter das Dach getragen.\n";

const HUMAN: &str = "Mara";

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(BOOK, "glas.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

async fn current(store: &Store, work: WorkId, node: usize) -> (u32, String) {
    let loaded = store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    let rev = &n.revisions[n.current_index().expect("aktuelle Fassung")];
    (rev.number, rev.body.clone())
}

async fn amend_seq(store: &Store, work: WorkId, node: usize, rev: u32) -> u32 {
    let tree = store.revision_tree(work).await.unwrap();
    tree.into_iter()
        .find(|n| n.node == node)
        .unwrap()
        .revisions
        .into_iter()
        .find(|r| r.number == rev)
        .unwrap()
        .amend_seq
}

#[tokio::test]
async fn updating_the_current_version_over_the_tree_writes_a_step_and_raises_the_stand() {
    let (_dir, store, work) = fresh().await;
    let (rev, before) = current(&store, work, 0).await;
    assert_eq!(amend_seq(&store, work, 0, rev).await, 0, "Testvoraussetzung: nie fortgeschrieben");

    // Erst im Editor fortschreiben (Stand 1), dann über den Baum ändern.
    let amended = before.replacen("Morgen", "frühen Morgen", 1);
    let (change, reason) =
        store.save_human_at(work, 0, &amended, HUMAN, Some(rev), Some(0), SaveMode::Auto).await.unwrap();
    assert_eq!(reason, None, "kleine Änderung des Menschen auf eigener Fassung: fortgeschrieben");
    assert_eq!(change.unwrap().kind, ChangeKind::Amended, "Testvoraussetzung: Fortschreibung");
    assert_eq!(amend_seq(&store, work, 0, rev).await, 1, "Stand 1 nach der Fortschreibung");

    let over_tree = amended.replacen("im Kreis", "im Halbkreis", 1);
    let edit = RevisionEdit { body: Some(&over_tree), note: Some("vom Baum") };
    let done = store.update_revision(work, 0, rev, &edit, HUMAN).await.unwrap().expect("geändert");
    assert_eq!(done.kind, ChangeKind::Updated, "es bleibt ein Ändern über den Baum (Protokoll im Spiegel wie bisher)");
    assert_eq!(done.amend_seq, Some(2), "die Änderung nennt den neuen Stand");
    assert_eq!(amend_seq(&store, work, 0, rev).await, 2, "der Stand der Fassung ist gestiegen");
    assert_eq!(current(&store, work, 0).await, (rev, over_tree.clone()), "dieselbe Fassung trägt den neuen Text");

    let steps = store.amend_history(work, 0, rev).await.unwrap();
    assert_eq!(steps.len(), 2, "zwei Schritte: Fortschreibung und Ändern über den Baum: {steps:#?}");
    assert_eq!(
        (steps[1].seq, steps[1].kind, steps[1].undo_of),
        (2, AmendKind::Amend, None),
        "der neue Schritt hat die Art 'amend' (kein Umbau des CHECK nötig)"
    );
    assert_eq!(steps[1].body_before, amended, "Text vorher ist der Stand 1");
    assert_eq!(steps[1].body_after, over_tree, "Text nachher ist der neue");
    assert_eq!(
        steps[0].body_after, steps[1].body_before,
        "die Kette der Schritte ist lückenlos (Vorspulen, Austausch)"
    );
    assert_eq!(steps[1].author, HUMAN, "der Mensch steht am Schritt");

    // Rückgängig über den Verlauf geht auch für diesen Schritt (als neuer Schritt, nie durch Löschen).
    let undone = store.amend_undo(work, 0, rev, 2, HUMAN).await.unwrap();
    assert_eq!(undone.amend_seq, Some(3), "die Rücknahme ist Schritt 3");
    assert_eq!(current(&store, work, 0).await.1, amended, "der Text vor dem Ändern über den Baum steht wieder da");
}

#[tokio::test]
async fn a_stale_editor_no_longer_overwrites_a_change_made_over_the_tree() {
    // Codereview store-1: Reiter A lädt Fassung n mit Stand 0; im Baum (Reiter B) ändert der Mensch den Text.
    let (_dir, store, work) = fresh().await;
    let (rev, loaded_by_a) = current(&store, work, 0).await;
    let from_b = loaded_by_a.replacen("der erste Schnee", "der erste nasse Schnee", 1);
    let edit = RevisionEdit { body: Some(&from_b), note: None };
    store.update_revision(work, 0, rev, &edit, HUMAN).await.unwrap().expect("B ändert");

    // A speichert eine kleine Änderung auf seinem alten Text, mit dem Stand, den er geladen hatte.
    let from_a = loaded_by_a.replacen("Morgen", "Mittag", 1);
    let (change, reason) =
        store.save_human_at(work, 0, &from_a, HUMAN, Some(rev), Some(0), SaveMode::Auto).await.unwrap();
    let change = change.expect("gespeichert");
    assert_eq!(
        reason,
        Some(NewReason::NotOnCurrent),
        "die Grundlage von A ist nicht mehr aktuell: neue Fassung statt stiller Fortschreibung"
    );
    assert_ne!(change.rev, rev, "A bekommt eine neue Fassung");
    let loaded = store.load_work(work).await.unwrap();
    let kept = loaded.nodes[0].revisions.iter().find(|r| r.number == rev).unwrap();
    assert_eq!(kept.body, from_b, "die Änderung aus dem Baum steht unversehrt in ihrer Fassung");
    assert_eq!(kept.status, RevisionStatus::Superseded, "sie ist ersetzt, nicht überschrieben");
}

#[tokio::test]
async fn update_with_an_outdated_stand_is_refused_and_changes_nothing() {
    let (_dir, store, work) = fresh().await;
    let (rev, body) = current(&store, work, 1).await;
    let first = body.replacen("gewaschen", "zweimal gewaschen", 1);
    let edit = RevisionEdit { body: Some(&first), note: None };
    let done = store.update_revision_at(work, 1, rev, &edit, HUMAN, Some(0)).await.unwrap().expect("geändert");
    assert_eq!(done.amend_seq, Some(1), "auf der aktuellen Fassung entsteht Stand 1");

    // Ein zweites Formular, das noch Stand 0 zeigt (Codereview web-3).
    let late = body.replacen("vom Fluss", "aus der Grube", 1);
    let edit = RevisionEdit { body: Some(&late), note: None };
    let err = store.update_revision_at(work, 1, rev, &edit, HUMAN, Some(0)).await.unwrap_err();
    assert!(
        matches!(&err, StoreError::Stale(m) if m.starts_with("Veraltet: ") && m.contains("Stand 1")),
        "veralteter Stand → Stale mit „Veraltet: …“ und dem jetzigen Stand, war {err:?}"
    );
    assert_eq!(current(&store, work, 1).await.1, first, "der neuere Text ist nicht überschrieben");
    assert_eq!(store.amend_history(work, 1, rev).await.unwrap().len(), 1, "kein weiterer Schritt");

    // Mit dem richtigen Stand und ohne Angabe (alte Clients) geht es weiter.
    let ok = store.update_revision_at(work, 1, rev, &edit, HUMAN, Some(1)).await.unwrap().expect("geändert");
    assert_eq!(ok.amend_seq, Some(2), "mit dem aktuellen Stand wird geändert");
    let note_only = RevisionEdit { body: None, note: Some("nur Notiz") };
    let noted = store.update_revision(work, 1, rev, &note_only, HUMAN).await.unwrap().expect("Notiz geändert");
    assert_eq!(noted.amend_seq, None, "nur die Notiz: kein Schritt, der Stand bleibt");
    assert_eq!(amend_seq(&store, work, 1, rev).await, 2, "Stand unverändert nach der Notiz");
}

#[tokio::test]
async fn a_never_amended_proposal_gets_no_history_when_the_human_edits_it() {
    let (_dir, store, work) = fresh().await;
    let (_, body) = current(&store, work, 1).await;
    let proposal = body.replacen("gesiebt", "fein gesiebt", 1);
    let number = store.add_proposal(work, 1, &proposal, "@lektor", "probe-1", "").await.unwrap();
    let edited = proposal.replacen("Säcken", "Leinensäcken", 1);
    let edit = RevisionEdit { body: Some(&edited), note: None };
    let done = store.update_revision(work, 1, number, &edit, HUMAN).await.unwrap().expect("geändert");
    assert_eq!(done.amend_seq, None, "ein Vorschlag (nicht aktuell, nie fortgeschrieben) bekommt keinen Verlauf");
    assert!(store.amend_history(work, 1, number).await.unwrap().is_empty(), "kein Schritt am Vorschlag");
    assert_eq!(amend_seq(&store, work, 1, number).await, 0, "Stand bleibt 0");
}

// ---- Befunde -------------------------------------------------------------------------------------------------------

fn post(body: &str) -> Post {
    Post { id: 0, created: String::new(), author: "@lektor".into(), body: body.into(), sources: vec![], refs: vec![] }
}

async fn finding(store: &Store, work: WorkId, key: i64, body: &str, summary_of: &[String]) -> baruch_store::Finding {
    let answer = store.add_post(work, 0, &post(body)).await.unwrap();
    store
        .finding_insert(&NewFinding {
            work,
            key: Some(key),
            post: Some(answer),
            thread_post: None,
            position: 0,
            persona: "@lektor",
            kind: FindingKind::Hint,
            body,
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn summaries_do_not_count_as_open_and_turn_stale_when_all_origins_are_decided() {
    let (_dir, store, work) = fresh().await;
    let key = store.load_work(work).await.unwrap().nodes[0].key;
    let a = finding(&store, work, key, "Hinweis: Der Ofen braucht ein Bild.", &[]).await;
    let b = finding(&store, work, key, "Hinweis: Der Schnee kommt zu früh.", &[]).await;
    let summary =
        finding(&store, work, key, "Sammlung: Bild am Ofen, Schnee später.", &[a.uuid.clone(), b.uuid.clone()]).await;
    assert_eq!(summary.summary_of.len(), 2, "Testvoraussetzung: ein Sammelbefund zu zwei Ursprüngen");
    assert!(!summary.stale_summary, "solange Ursprünge offen sind, ist der Sammelbefund nicht veraltet");

    let open = store.finding_open_counts(work).await.unwrap();
    assert_eq!(
        open.get(&key).copied(),
        Some(2),
        "„k offen“ zählt die zwei Ursprünge, nicht zusätzlich den Sammelbefund (PM-Entscheidung, sonst doppelt)"
    );

    store
        .finding_set_status(work, a.id, &StatusChange::Rejected { reason: "passt nicht".into() }, HUMAN)
        .await
        .unwrap();
    let half = store.finding_get(work, summary.id).await.unwrap();
    assert!(!half.stale_summary, "ein Ursprung ist noch offen: nicht veraltet");
    store.finding_set_status(work, b.id, &StatusChange::Done { reason: String::new() }, HUMAN).await.unwrap();

    let list = store.finding_list(work, &FindingFilter::default()).await.unwrap();
    let listed = list.iter().find(|f| f.id == summary.id).unwrap();
    assert!(listed.stale_summary, "alle Ursprünge entschieden: der offene Sammelbefund ist veraltet (in der Liste)");
    assert_eq!(listed.status, FindingStatus::Open, "sein Status bleibt, wie der Mensch ihn gesetzt hat");
    assert!(store.finding_get(work, summary.id).await.unwrap().stale_summary, "ebenso beim einzelnen Lesen");
    assert!(
        list.iter().filter(|f| f.id != summary.id).all(|f| !f.stale_summary),
        "gewöhnliche Befunde sind nie „veraltet“"
    );
    assert_eq!(store.finding_open_counts(work).await.unwrap().get(&key), None, "kein offener Befund zählt mehr");

    // Ein entschiedener Sammelbefund ist nicht mehr „veraltet“ (das Kennzeichen gilt nur für offene).
    store.finding_set_status(work, summary.id, &StatusChange::Done { reason: String::new() }, HUMAN).await.unwrap();
    assert!(!store.finding_get(work, summary.id).await.unwrap().stale_summary, "entschieden: kein Kennzeichen");
}

#[tokio::test]
async fn a_list_carries_the_latest_open_proposal_of_every_finding() {
    let (_dir, store, work) = fresh().await;
    let key = store.load_work(work).await.unwrap().nodes[1].key;
    let lektor = IdeaOrigin::Persona { handle: "@lektor".into() };
    let mut ids = Vec::new();
    for n in 0..7 {
        ids.push(finding(&store, work, key, &format!("Hinweis {n}: Sand prüfen."), &[]).await.id);
    }
    let done = StatusChange::Done { reason: String::new() };
    // Befund 0: zwei Vorschläge (der jüngste zählt); Befund 3: einer; Befund 5: einer, vom Menschen abgelehnt.
    store.finding_propose_status(work, ids[0], &done, &lektor, "erste Notiz").await.unwrap();
    // Derselbe Vorschlag doppelt wird abgelehnt; der zweite schlägt darum „übernommen in Fassung n“ vor.
    let (rev, _) = current(&store, work, 1).await;
    let adopted = StatusChange::Adopted { rev };
    let latest = store.finding_propose_status(work, ids[0], &adopted, &lektor, "zweite Notiz").await.unwrap();
    let single = store.finding_propose_status(work, ids[3], &done, &lektor, "nur eine").await.unwrap();
    let refused = store.finding_propose_status(work, ids[5], &done, &lektor, "abgelehnt").await.unwrap();
    store.finding_decide(work, refused.id, false, HUMAN).await.unwrap();

    let list = store.finding_list(work, &FindingFilter::default()).await.unwrap();
    assert_eq!(list.len(), 7, "alle sieben Befunde");
    let pending = |id: i64| list.iter().find(|f| f.id == id).unwrap().pending.clone();
    assert_eq!(pending(ids[0]).map(|p| p.id), Some(latest.id), "der jüngste offene Vorschlag, nicht der erste");
    assert_eq!(pending(ids[3]).map(|p| p.note), Some(single.note), "ein einzelner Vorschlag kommt mit");
    assert_eq!(pending(ids[5]), None, "ein entschiedener Vorschlag ist kein offener");
    assert_eq!(pending(ids[1]), None, "ohne Vorschlag bleibt das Feld leer");
    for f in &list {
        assert_eq!(
            store.finding_get(work, f.id).await.unwrap().pending,
            f.pending,
            "Befund {}: die Liste (eine Abfrage) und das einzelne Lesen liefern denselben Vorschlag",
            f.id
        );
    }
}

// ---- Karte des Exports ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn lines_of_the_book_end_do_not_belong_to_the_last_section() {
    let (_dir, store, work) = fresh().await;
    let plain = store.export_map(work).await.unwrap();
    assert_eq!(plain.tail_first_line(), None, "ohne Register und Glossar hat der Export kein Buchende");
    let last = plain.sections.last().unwrap().clone();
    let lines = plain.typst.lines().count();
    assert_eq!(
        plain.section_at_line(lines).map(|s| s.key),
        Some(last.key),
        "ohne Buchende gehört die letzte Zeile weiter zum letzten Abschnitt"
    );

    // Dieselbe Karte mit einem Buchende dahinter (wie `append_tail` es anhängt).
    let mut with_tail = plain.clone();
    with_tail.typst.push_str("\n#pagebreak()\n= Register\n#baruch-register()\n");
    let first = with_tail.tail_first_line().expect("Buchende vorhanden");
    let tail_line = with_tail.typst.lines().position(|l| l == "#pagebreak()").unwrap() + 1;
    assert_eq!(first, tail_line, "das Buchende beginnt mit seiner ersten nicht leeren Zeile");
    assert_eq!(
        with_tail.section_at_line(first + 1),
        None,
        "eine Zeile im Buchende gehört keinem Abschnitt (vorher: dem letzten)"
    );
    assert!(with_tail.in_tail(first + 2) && !with_tail.in_tail(first - 1), "Grenze des Buchendes");
    assert_eq!(
        with_tail.section_at_line(last.first_line).map(|s| s.key),
        Some(last.key),
        "die Zeilen des letzten Abschnitts bleiben seine"
    );
    assert_eq!(with_tail.section_at_line(1), None, "Zeile 1 bleibt Werkvorlage");
    assert!(!with_tail.in_tail(1), "und sie ist kein Buchende");
}
