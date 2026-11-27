// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fortschreiben der aktuellen Fassung (Runde 22, Vertrag §2.1–2.6) auf echten SQLite-Dateien im Temp-Ordner, mit
//! einem frei erfundenen Werk: Fortschreiben im Modus `Auto` (Editor und `revision.save`), neue Fassung mit Grund in der
//! Reihenfolge aus §2.2, Verlauf und Rückgängig als neuer Schritt, Stand der Basis an Vorschlägen, Einstellung
//! „Fassungen“ und Grabsteine beim Löschen.

use baruch_core::{Author, RevisionStatus};
use baruch_store::{
    AmendKind, ChangeKind, NewReason, Restructure, RevisionInfo, RevisionPolicy, SaveMode, Store, StoreError,
};
use sqlx::SqlitePool;

const BOOK: &str = "#set document(title: \"Das Haus am Deich\", author: \"Jona Brandt\")\n\n\
= Ankunft\n\nAm Abend erreichte die Familie das alte Haus am Deich. Der Wind trug Salz und Möwenrufe über die \
Wiesen, und im Fenster brannte ein schwaches Licht, das niemand angezündet hatte.\n\n\
= Sturm\n\nIn der dritten Nacht kam der Sturm. Die Fensterläden schlugen, das Wasser stieg bis an den Fuß des \
Deiches, und alle saßen wach in der Küche.\n";

const HUMAN: &str = "Jona";

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let imported = store.import_typst(BOOK, "haus.typ", HUMAN).await.unwrap();
    (dir, store, imported.work)
}

/// Aktueller Text des Abschnitts `node`.
async fn current_body(store: &Store, work: i64, node: usize) -> String {
    let loaded = store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    n.revisions[n.current_index().expect("aktuelle Fassung")].body.clone()
}

/// Text mit einem kleinen Zusatz im ersten Absatz (ein Wort vor „Licht“, weit unter 20 %).
fn touched(body: &str, word: &str) -> String {
    let changed = body.replacen("Licht", &format!("{word} Licht"), 1);
    assert_ne!(changed, body, "Testtext enthält „Licht“");
    changed
}

async fn infos(store: &Store, work: i64, node: usize) -> Vec<RevisionInfo> {
    store.revision_tree(work).await.unwrap().into_iter().find(|n| n.node == node).unwrap().revisions
}

async fn currents(pool: &SqlitePool, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND n.seq = ? AND r.status = 'current'",
    )
    .bind(work)
    .bind(seq)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Anlagezeit einer Fassung in die Vergangenheit setzen (ohne Schlafen). Die Herkunft ist per Trigger unveränderlich;
/// der Test nimmt den Trigger dafür kurz heraus und setzt ihn wörtlich wieder ein.
async fn age_revision(pool: &SqlitePool, work: i64, number: u32) {
    let trigger: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'revision_origin_immutable'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("DROP TRIGGER revision_origin_immutable").execute(&mut *conn).await.unwrap();
    sqlx::query(
        "UPDATE revision SET created = '2000-01-01T00:00:00.000Z' WHERE number = ? \
         AND node_id = (SELECT id FROM node WHERE work_id = ? AND seq = 0)",
    )
    .bind(i64::from(number))
    .bind(work)
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(trigger.as_str())).execute(&mut *conn).await.unwrap();
}

async fn editor_save(store: &Store, work: i64, body: &str, based_on: u32) -> baruch_store::OutlineSave {
    store.outline_save_mode(work, 0, body, Some(based_on), Restructure::default(), HUMAN, SaveMode::Auto).await.unwrap()
}

#[tokio::test]
async fn saving_in_the_editor_amends_the_own_current_version_and_records_each_step() {
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    let first = touched(&original, "gelbes");
    let saved = editor_save(&store, work, &first, 1).await;
    let change = saved.change.expect("Text geändert");
    assert_eq!(change.kind, ChangeKind::Amended, "kleine eigene Änderung auf der aktuellen Fassung: fortgeschrieben");
    assert_eq!((change.rev, change.amend_seq), (1, Some(1)), "Fassung 1, Stand 1");
    assert_eq!(saved.new_reason, None, "kein Grund für eine neue Fassung");
    assert!(saved.outline.is_none(), "Überschrift unverändert: keine Gliederungsänderung");

    let second = touched(&first, "flackerndes");
    let change = editor_save(&store, work, &second, 1).await.change.expect("zweiter Schritt");
    assert_eq!((change.kind, change.amend_seq), (ChangeKind::Amended, Some(2)), "zweite Fortschreibung: Stand 2");

    let loaded = store.load_work(work).await.unwrap();
    assert_eq!(loaded.nodes[0].revisions.len(), 1, "keine neue Fassung entstanden");
    assert_eq!(current_body(&store, work, 0).await, second, "Text der Fassung ersetzt");
    let info = &infos(&store, work, 0).await[0];
    assert_eq!(info.number, 1, "Nummer bleibt");
    assert_eq!(info.status, RevisionStatus::Current, "Status bleibt aktuell");
    assert_eq!(info.author, Author::Human { name: HUMAN.into() }, "Herkunft bleibt");
    assert_eq!(info.based_on, None, "based_on bleibt");
    assert_eq!(info.edited, None, "edited setzt nur das Ändern über den Baum, nicht das Fortschreiben");
    assert_eq!(info.amend_seq, 2, "RevisionInfo trägt den Stand");
    assert_eq!(currents(store.pool(), work, 0).await, 1, "genau eine aktuelle Fassung je Abschnitt");

    let steps = store.amend_history(work, 0, 1).await.unwrap();
    assert_eq!(steps.iter().map(|s| s.seq).collect::<Vec<_>>(), [1, 2], "alle Schritte in Reihenfolge");
    assert!(steps.iter().all(|s| s.kind == AmendKind::Amend && s.undo_of.is_none()), "beides Fortschreibungen");
    assert_eq!((steps[0].body_before.as_str(), steps[0].body_after.as_str()), (original.as_str(), first.as_str()));
    assert_eq!((steps[1].body_before.as_str(), steps[1].body_after.as_str()), (first.as_str(), second.as_str()));
    assert!(steps.iter().all(|s| s.author == HUMAN), "Autor jedes Schritts ist der Mensch");
    assert!(steps.iter().all(|s| s.uuid.len() == 36), "jeder Schritt hat eine uuid (Austausch, Grabstein)");
    assert_ne!(steps[0].uuid, steps[1].uuid, "uuids verschieden");

    // Gleicher Text: nichts geschrieben.
    let same = editor_save(&store, work, &second, 1).await;
    assert!(same.change.is_none() && same.new_reason.is_none(), "unverändert: weder Schritt noch neue Fassung");
    assert_eq!(store.amend_history(work, 0, 1).await.unwrap().len(), 2, "kein leerer Schritt");
}

#[tokio::test]
async fn revision_save_in_auto_mode_amends_too_and_new_mode_keeps_the_old_behaviour() {
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    let (change, reason) = store
        .save_human_with_reason(work, 0, &touched(&original, "blasses"), HUMAN, None, SaveMode::Auto)
        .await
        .unwrap();
    let change = change.expect("geändert");
    assert_eq!((change.kind, change.amend_seq, reason), (ChangeKind::Amended, Some(1), None), "revision.save auto");

    let newer = touched(&original, "rotes");
    let change = store.save_human_mode(work, 0, &newer, HUMAN, Some(1), SaveMode::New).await.unwrap().unwrap();
    assert_eq!((change.kind, change.rev, change.amend_seq), (ChangeKind::Saved, 2, None), "Modus new: neue Fassung 2");

    // Die alten Funktionen legen wie bisher neue Fassungen an (mode fehlt = new).
    let change = store.save_human(work, 0, &touched(&newer, "kleines"), HUMAN).await.unwrap().unwrap();
    assert_eq!((change.kind, change.rev), (ChangeKind::Saved, 3), "save_human: neue Fassung 3");
    let body = current_body(&store, work, 0).await;
    let done =
        store.outline_save(work, 0, &touched(&body, "warmes"), Some(3), Restructure::default(), HUMAN).await.unwrap();
    assert_eq!(done.change.map(|c| (c.kind, c.rev)), Some((ChangeKind::Saved, 4)), "outline_save: neue Fassung 4");
    assert_eq!(done.new_reason, None, "ohne Modus kein Grund");
    let done = store
        .outline_save_mode(work, 0, &touched(&body, "helles"), Some(4), Restructure::default(), HUMAN, SaveMode::New)
        .await
        .unwrap();
    assert_eq!(done.change.map(|c| c.rev), Some(5), "outline_save_mode New: neue Fassung 5");
    assert_eq!(done.new_reason, None, "new_reason nur im Modus Auto");
    assert_eq!(currents(store.pool(), work, 0).await, 1, "genau eine aktuelle Fassung");
}

#[tokio::test]
async fn a_new_version_names_its_reason() {
    let (_dir, store, work) = fresh().await;
    // Die Grenzregeln (große Änderung, Zeit) gelten nur, wo sie eingeschaltet sind; die Vorgabe seit 2026-11-27 hat keine.
    let limited = RevisionPolicy { amend: true, max_change_percent: 20, new_after_minutes: 30 };
    store.set_revision_policy(work, &limited, HUMAN).await.unwrap();
    let original = current_body(&store, work, 0).await;

    // not_own: übernommener Vorschlag einer Persona ist die aktuelle Fassung.
    let proposal = store.add_proposal(work, 0, &touched(&original, "fremdes"), "@lektor", "mock", "").await.unwrap();
    store.accept(work, 0, proposal).await.unwrap();
    let body = current_body(&store, work, 0).await;
    let saved = editor_save(&store, work, &touched(&body, "eigenes"), proposal).await;
    assert_eq!(saved.new_reason, Some(NewReason::NotOwn), "Fassung der Persona wird nie fortgeschrieben");
    let own = saved.change.expect("neue Fassung");
    assert_eq!((own.kind, own.rev), (ChangeKind::Saved, 3), "neue Fassung 3 vom Menschen");

    // not_on_current: auf einer anderen Fassung (1) bearbeitet.
    let saved = editor_save(&store, work, &touched(&original, "altes"), 1).await;
    assert_eq!(saved.new_reason, Some(NewReason::NotOnCurrent), "Basis ist nicht die aktuelle Fassung");

    // heading_changed: neuer Titel.
    let current = infos(&store, work, 0).await.iter().find(|r| r.status == RevisionStatus::Current).unwrap().number;
    let body = current_body(&store, work, 0).await;
    let renamed = body.replacen("= Ankunft", "= Ankunft am Deich", 1);
    let saved = editor_save(&store, work, &renamed, current).await;
    assert_eq!(saved.new_reason, Some(NewReason::HeadingChanged), "Überschrift geändert");
    assert!(saved.outline.is_some(), "Umbenennen bleibt eine Gliederungsänderung");

    // large_change: derselbe Titel, ganz anderer Text.
    let current = saved.change.unwrap().rev;
    let rewritten = "= Ankunft am Deich\n\nNebel. Stille. Ein Hund bellt dreimal, dann nichts mehr.\n";
    let saved = editor_save(&store, work, rewritten, current).await;
    assert_eq!(saved.new_reason, Some(NewReason::LargeChange), "Änderung über 20 %");

    // time_elapsed: Zeitstempel der Fassung in die Vergangenheit (ohne Schlafen).
    let current = saved.change.unwrap().rev;
    age_revision(store.pool(), work, current).await;
    let body = current_body(&store, work, 0).await;
    let saved = editor_save(&store, work, &body.replacen("Hund", "Hund leise", 1), current).await;
    assert_eq!(saved.new_reason, Some(NewReason::TimeElapsed), "seit dem letzten Schreiben mehr als 30 min");

    // Eine Fortschreibung zählt als letztes Schreiben: alte Fassung, frischer Schritt → weiter fortschreiben.
    let current = saved.change.unwrap().rev;
    let body = current_body(&store, work, 0).await;
    let amended = editor_save(&store, work, &body.replacen("dreimal", "dreimal kurz", 1), current).await;
    assert_eq!(amended.change.as_ref().map(|c| c.kind), Some(ChangeKind::Amended), "frische Fassung: fortschreiben");
    age_revision(store.pool(), work, current).await;
    let body = current_body(&store, work, 0).await;
    let again = editor_save(&store, work, &body.replacen("bellt", "bellt heiser", 1), current).await;
    assert_eq!(
        again.change.map(|c| (c.kind, c.amend_seq)),
        Some((ChangeKind::Amended, Some(2))),
        "maßgeblich ist max(Anlage, letzte Fortschreibung): der Schritt eben zählt"
    );

    // policy_off: Fortschreiben im Werk ausgeschaltet.
    let off = RevisionPolicy { amend: false, ..RevisionPolicy::default() };
    store.set_revision_policy(work, &off, HUMAN).await.unwrap();
    let body = current_body(&store, work, 0).await;
    let saved = editor_save(&store, work, &body.replacen("Nebel.", "Nebel!", 1), current).await;
    assert_eq!(saved.new_reason, Some(NewReason::PolicyOff), "ausgeschaltet: immer neue Fassung");
    assert_eq!(currents(store.pool(), work, 0).await, 1, "nach allem genau eine aktuelle Fassung");
}

#[tokio::test]
async fn undo_restores_the_text_before_a_step_as_a_new_step_and_only_on_the_own_current_version() {
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    let first = touched(&original, "gelbes");
    let second = touched(&first, "müdes");
    editor_save(&store, work, &first, 1).await;
    editor_save(&store, work, &second, 1).await;

    let change = store.amend_undo(work, 0, 1, 1, HUMAN).await.expect("Rückgängig bis vor Schritt 1");
    assert_eq!((change.kind, change.amend_seq), (ChangeKind::AmendUndone, Some(3)), "neuer Schritt 3");
    assert_eq!(current_body(&store, work, 0).await, original, "Text wie vor Schritt 1");
    let steps = store.amend_history(work, 0, 1).await.unwrap();
    assert_eq!(steps.len(), 3, "nie werden Verlaufszeilen gelöscht");
    assert_eq!((steps[2].kind, steps[2].undo_of), (AmendKind::Undo, Some(1)), "Art undo mit undo_of = 1");
    assert_eq!((steps[2].body_before.as_str(), steps[2].body_after.as_str()), (second.as_str(), original.as_str()));

    let same = store.amend_undo(work, 0, 1, 1, HUMAN).await;
    assert!(matches!(same, Err(StoreError::Refused(m)) if m.contains("schon")), "gleicher Stand: nichts zu tun");
    let unknown = store.amend_undo(work, 0, 1, 9, HUMAN).await;
    assert!(matches!(unknown, Err(StoreError::Refused(m)) if m.contains("Schritt")), "unbekannter Schritt");

    // Nicht die aktuelle Fassung: abgelehnt.
    let newer = store.save_human_mode(work, 0, &second, HUMAN, Some(1), SaveMode::New).await.unwrap().unwrap();
    let not_current = store.amend_undo(work, 0, 1, 2, HUMAN).await;
    assert!(
        matches!(not_current, Err(StoreError::Refused(m)) if m.contains("aktuellen Fassung")),
        "Rückgängig nur auf der aktuellen Fassung"
    );
    // Fassung einer Persona als aktuelle: abgelehnt (dort gibt es ohnehin keine Schritte).
    let proposal = store.add_proposal(work, 0, &touched(&second, "fremdes"), "@lektor", "mock", "").await.unwrap();
    store.accept(work, 0, proposal).await.unwrap();
    let persona = store.amend_undo(work, 0, proposal, 1, HUMAN).await;
    assert!(
        matches!(persona, Err(StoreError::Refused(m)) if m.contains("Menschen")),
        "Rückgängig nur auf einer Fassung des Menschen"
    );
    assert!(newer.rev > 1, "neue Fassung angelegt");
    assert_eq!(store.amend_history(work, 0, 1).await.unwrap().len(), 3, "Verlauf von Fassung 1 bleibt vollständig");
}

#[tokio::test]
async fn proposals_remember_the_amend_state_of_their_base() {
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    let first = touched(&original, "gelbes");
    editor_save(&store, work, &first, 1).await;
    editor_save(&store, work, &touched(&first, "müdes"), 1).await;
    let plain = store.add_proposal(work, 0, &touched(&first, "fremdes"), "@lektor", "mock", "").await.unwrap();
    let on = store.add_proposal_on(work, 0, &touched(&first, "anderes"), "@lektor", "mock", "", 1, None).await.unwrap();
    let list = infos(&store, work, 0).await;
    let base_of = |n: u32| list.iter().find(|r| r.number == n).unwrap().based_on_amend;
    assert_eq!(base_of(plain), Some(2), "add_proposal merkt den Stand 2 der Basis");
    assert_eq!(base_of(on), Some(2), "add_proposal_on ebenso");
    assert_eq!(base_of(1), None, "die Fassung des Menschen selbst hat keinen Basis-Stand");

    // Weitere Fortschreibung ändert den gemerkten Stand nicht.
    let body = current_body(&store, work, 0).await;
    editor_save(&store, work, &touched(&body, "drittes"), 1).await;
    let list = infos(&store, work, 0).await;
    assert_eq!(
        list.iter().find(|r| r.number == plain).unwrap().based_on_amend,
        Some(2),
        "der Vorschlag beruht weiter auf Stand 2"
    );
}

#[tokio::test]
async fn the_policy_has_defaults_limits_and_goes_with_the_work() {
    let (_dir, store, work) = fresh().await;
    assert_eq!(store.revision_policy(work).await.unwrap(), RevisionPolicy::default(), "Vorgabe ohne Zeile");
    assert_eq!(
        RevisionPolicy::default(),
        RevisionPolicy { amend: true, max_change_percent: 100, new_after_minutes: 0 },
        "Vorgabe seit 2026-11-27: an, ohne Größen- und Zeitgrenze (Speichern schreibt immer fort)"
    );
    let wanted = RevisionPolicy { amend: true, max_change_percent: 35, new_after_minutes: 10 };
    let change = store.set_revision_policy(work, &wanted, HUMAN).await.unwrap();
    assert_eq!(change.changed, ["max_change_percent", "new_after_minutes"], "geänderte Felder");
    assert_eq!((change.before, change.after), (RevisionPolicy::default(), wanted), "vorher/nachher");
    assert_eq!(store.revision_policy(work).await.unwrap(), wanted, "gespeichert");
    let again = store.set_revision_policy(work, &wanted, HUMAN).await.unwrap();
    assert!(again.changed.is_empty(), "nichts geändert");
    for (bad, part) in [
        (RevisionPolicy { max_change_percent: 0, ..wanted }, "1 bis 100"),
        (RevisionPolicy { max_change_percent: 101, ..wanted }, "1 bis 100"),
        (RevisionPolicy { new_after_minutes: 1441, ..wanted }, "0 bis 1440"),
    ] {
        let refused = store.set_revision_policy(work, &bad, HUMAN).await;
        assert!(matches!(refused, Err(StoreError::Refused(m)) if m.contains(part)), "Grenze {part}: {bad:?}");
    }
    assert!(matches!(store.revision_policy(999).await, Err(StoreError::UnknownWork(999))), "unbekanntes Werk");
    let title = store.load_work(work).await.unwrap().title;
    store.delete_work(work, &title).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_revision_policy WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0, "work_revision_policy geht mit dem Werk");
}

#[tokio::test]
async fn deleting_a_version_or_the_work_leaves_tombstones_of_the_steps_with_the_work_uuid() {
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    editor_save(&store, work, &touched(&original, "gelbes"), 1).await;
    let steps = store.amend_history(work, 0, 1).await.unwrap();
    // Neue Fassung, dann Fassung 1 (mit Schritt) löschen.
    store.save_human_mode(work, 0, &touched(&original, "rotes"), HUMAN, Some(1), SaveMode::New).await.unwrap();
    store.delete_revision(work, 0, 1, HUMAN).await.unwrap();
    let work_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    let grave: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&steps[0].uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(grave, ("revision_amend".into(), Some(work_uuid.clone())), "Grabstein des Schritts mit Werk-uuid");

    // Werk löschen: Schritte der jetzt aktuellen Fassung hinterlassen ebenfalls Grabsteine.
    let current = infos(&store, work, 0).await.iter().find(|r| r.status == RevisionStatus::Current).unwrap().number;
    let body = current_body(&store, work, 0).await;
    editor_save(&store, work, &touched(&body, "spätes"), current).await;
    let last = store.amend_history(work, 0, current).await.unwrap();
    let title = store.load_work(work).await.unwrap().title;
    store.delete_work(work, &title).await.unwrap();
    let grave: Option<String> = sqlx::query_scalar("SELECT work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(&last[0].uuid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(grave.as_deref(), Some(work_uuid.as_str()), "Grabstein beim Löschen des Werks kennt die Werk-uuid");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM revision_amend").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "keine Schritte bleiben übrig");
}

#[tokio::test]
async fn a_stale_editor_on_an_amended_version_gives_a_new_version_instead_of_silently_overwriting() {
    // Befund (Nachbesserung R22): Reiter A und B öffnen Fassung 1 mit Stand 0. A schreibt fort (Stand 1). B speichert
    // danach eine kleine Änderung auf seinem alten Text. Die Basis von B ist nicht mehr aktuell (anderer Stand): Es
    // entsteht eine neue Fassung, beide Texte bleiben als Fassungen sichtbar – die Änderung von A verschwindet nicht.
    let (_dir, store, work) = fresh().await;
    let original = current_body(&store, work, 0).await;
    let tab_a = touched(&original, "gelbes");
    let tab_b = touched(&original, "blaues");
    let saved = store
        .outline_save_at(work, 0, &tab_a, Some(1), Some(0), Restructure::default(), HUMAN, SaveMode::Auto)
        .await
        .unwrap();
    let change = saved.change.expect("A geändert");
    assert_eq!((change.kind, change.amend_seq), (ChangeKind::Amended, Some(1)), "A schreibt auf Stand 1 fort");

    let saved = store
        .outline_save_at(work, 0, &tab_b, Some(1), Some(0), Restructure::default(), HUMAN, SaveMode::Auto)
        .await
        .unwrap();
    assert_eq!(
        saved.new_reason,
        Some(NewReason::NotOnCurrent),
        "B auf Stand 0 einer inzwischen fortgeschriebenen Fassung"
    );
    let change = saved.change.expect("B gespeichert");
    assert_eq!(change.rev, 2, "B wird Fassung 2 (keine Fortschreibung)");
    let loaded = store.load_work(work).await.unwrap();
    let bodies: Vec<&str> = loaded.nodes[0].revisions.iter().map(|r| r.body.as_str()).collect();
    assert!(bodies.contains(&tab_a.as_str()), "der Text von A bleibt als Fassung 1 sichtbar: {bodies:?}");
    assert!(bodies.contains(&tab_b.as_str()), "der Text von B ist Fassung 2: {bodies:?}");
    assert_eq!(store.amend_history(work, 0, 1).await.unwrap().len(), 1, "Fassung 1 behält genau den Schritt von A");

    // Mit dem richtigen Stand (der Editor nach dem Neuzeichnen) wird weiter fortgeschrieben.
    let next = touched(&tab_b, "kleines");
    let saved = store
        .outline_save_at(work, 0, &next, Some(2), Some(0), Restructure::default(), HUMAN, SaveMode::Auto)
        .await
        .unwrap();
    assert_eq!(saved.change.map(|c| (c.kind, c.rev)), Some((ChangeKind::Amended, 2)), "aktueller Stand: fortschreiben");

    // Derselbe Fall über `revision.save` (Store::save_human_at).
    let stale = touched(&tab_b, "großes");
    let (change, reason) = store.save_human_at(work, 0, &stale, HUMAN, Some(2), Some(0), SaveMode::Auto).await.unwrap();
    assert_eq!(reason, Some(NewReason::NotOnCurrent), "revision.save mit veraltetem Stand: neue Fassung");
    assert_eq!(change.map(|c| c.rev), Some(3), "als Fassung 3");
}
