// SPDX-License-Identifier: AGPL-3.0-or-later
//! Befunde im Store (Runde 28, Migration 0046). S1 prüft die voll umgesetzten Abfragen (`finding_insert`,
//! `finding_get`, `finding_list`, `finding_history`, `finding_open_counts`, `findings_adopted_in`); befunde (R28a)
//! prüft hier Statuswechsel, Vorschläge, Entscheidungen, `befunde.md` und Grabsteine. Echte
//! SQLite-Datei im Temp-Ordner, Testtexte frei erfunden.

use baruch_core::Post;
use baruch_store::{
    DeferTarget, FindingFilter, FindingKind, FindingStatus, NewFinding, StatusChange, Store, StoreError, WorkId,
};

const HAFEN: &str = "\
= Der Hafen

Möwen über dem Kai.

= Die Werft

Ein Rumpf aus Eiche.
";

async fn harbour() -> (tempfile::TempDir, Store, WorkId, Vec<i64>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(HAFEN, "hafen.typ", "Ida").await.unwrap().work;
    let keys = store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    (dir, store, work, keys)
}

fn post(author: &str, body: &str) -> Post {
    Post { id: 0, created: String::new(), author: author.into(), body: body.into(), sources: vec![], refs: vec![] }
}

fn new<'a>(work: WorkId, key: i64, post: i64, position: u32, kind: FindingKind, body: &'a str) -> NewFinding<'a> {
    NewFinding {
        work,
        key: Some(key),
        post: Some(post),
        thread_post: None,
        position,
        persona: "@lektor",
        kind,
        body,
        quote: "",
        quote_check: "",
        similar_to: None,
        summary_of: &[],
    }
}

#[tokio::test]
async fn inserted_findings_are_read_back_and_counted_as_open_by_the_contract_rule() {
    let (_dir, store, work, keys) = harbour().await;
    let answer = store
        .add_post(work, 0, &post("@lektor", "Was trägt: die Möwen.\nKritisch: „Kai“ steht allein."))
        .await
        .unwrap();
    let praise = store.finding_insert(&new(work, keys[0], answer, 0, FindingKind::Praise, "die Möwen")).await.unwrap();
    let critical = store
        .finding_insert(&NewFinding {
            quote: "Kai",
            ..new(work, keys[0], answer, 1, FindingKind::Critical, "„Kai“ steht allein.")
        })
        .await
        .unwrap();
    let attached = store
        .finding_insert(&NewFinding {
            similar_to: Some(critical.id),
            ..new(work, keys[0], answer, 2, FindingKind::Hint, "„Kai“ steht allein.")
        })
        .await
        .unwrap();
    assert_eq!(
        (critical.key, critical.node, critical.post),
        (Some(keys[0]), Some(0), Some(answer)),
        "Bezüge aufgelöst"
    );
    assert_eq!((critical.status, critical.quote.as_str()), (FindingStatus::Open, "Kai"), "offen, mit Zitat");
    assert_eq!(critical.uuid.len(), 36, "uuid vom Fülltrigger");
    assert!(critical.pending.is_none() && critical.adopted_rev.is_none(), "kein Vorschlag, keine Fassung");
    assert_eq!(store.finding_get(work, critical.id).await.unwrap(), critical, "finding_get liest dasselbe");

    let open = store.finding_open_counts(work).await.unwrap();
    assert_eq!(open.get(&keys[0]), Some(&1), "Lob und angehängte Befunde zählen nicht als offen (Vertrag §1.5)");
    assert_eq!(open.get(&keys[1]), None, "Abschnitte ohne offene Befunde fehlen");

    let listed = store.finding_list(work, &FindingFilter::default()).await.unwrap();
    assert_eq!(
        listed.iter().map(|f| f.id).collect::<Vec<_>>(),
        vec![critical.id],
        "Vorgabe: ohne Lob, ohne angehängte"
    );
    let all = FindingFilter { include_praise: true, include_attached: true, ..FindingFilter::default() };
    let ids: Vec<i64> = store.finding_list(work, &all).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(ids, vec![praise.id, critical.id, attached.id], "alle, älteste zuerst");
    let only_praise = FindingFilter { kinds: vec![FindingKind::Praise], ..FindingFilter::default() };
    let ids: Vec<i64> = store.finding_list(work, &only_praise).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(ids, vec![praise.id], "ausdrücklich nach Lob gefragt");
    let elsewhere = FindingFilter { keys: vec![keys[1]], include_praise: true, ..FindingFilter::default() };
    assert!(store.finding_list(work, &elsewhere).await.unwrap().is_empty(), "Filter nach Abschnitt");
    let done = FindingFilter { statuses: vec![FindingStatus::Done], ..FindingFilter::default() };
    assert!(store.finding_list(work, &done).await.unwrap().is_empty(), "Filter nach Status");
    assert!(store.finding_history(work, critical.id).await.unwrap().is_empty(), "die Erstanlage hat keinen Verlauf");
    assert!(store.findings_adopted_in(work, keys[0]).await.unwrap().is_empty(), "noch nichts übernommen");
}

#[tokio::test]
async fn a_capped_list_keeps_the_newest_findings_and_the_count_shows_the_cut() {
    // Über der Grenze fielen früher still die jüngsten Befunde weg (ORDER BY created LIMIT n); Matrix, Prompt
    // (Entscheidungen) und Ähnlichkeitsabgleich sahen dann gerade die neuesten nicht. Jetzt: die neuesten n,
    // weiter älteste zuerst geliefert, und `finding_count` zählt ohne Grenze.
    let (_dir, store, work, keys) = harbour().await;
    let answer = store.add_post(work, 0, &post("@lektor", "Hinweise.")).await.unwrap();
    let bodies = ["eins", "zwei", "drei", "vier", "fünf"];
    let mut ids = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        let f = store.finding_insert(&new(work, keys[0], answer, i as u32, FindingKind::Hint, body)).await.unwrap();
        ids.push(f.id);
    }
    let capped = FindingFilter { limit: Some(3), ..FindingFilter::default() };
    let listed: Vec<i64> = store.finding_list(work, &capped).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(listed, ids[2..].to_vec(), "die neuesten drei, älteste zuerst");
    assert_eq!(store.finding_count(work, &capped).await.unwrap(), 5, "gezählt ohne Grenze: gekappt ist erkennbar");
    let elsewhere = FindingFilter { keys: vec![keys[1]], ..FindingFilter::default() };
    assert_eq!(store.finding_count(work, &elsewhere).await.unwrap(), 0, "derselbe Filter wie die Liste");

    // finding.get lädt Angehängte und Ursprünge gezielt statt aus einer gekappten Werkliste.
    let attached = store
        .finding_insert(&NewFinding {
            similar_to: Some(ids[0]),
            ..new(work, keys[0], answer, 9, FindingKind::Hint, "eins!")
        })
        .await
        .unwrap();
    let got: Vec<i64> = store.finding_attached_to(work, ids[0]).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(got, vec![attached.id], "angehängt an den ältesten Befund");
    let first = store.finding_get(work, ids[0]).await.unwrap();
    let second = store.finding_get(work, ids[1]).await.unwrap();
    let uuids = vec![second.uuid.clone(), first.uuid.clone(), "unbekannt".to_owned()];
    let got: Vec<i64> = store.findings_by_uuid(work, &uuids).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(got, vec![ids[0], ids[1]], "per UUID, älteste zuerst, unbekannte fehlen");
}

#[tokio::test]
async fn findings_belong_to_their_work_and_limits_are_finding_errors() {
    let (_dir, store, work, keys) = harbour().await;
    let answer = store.add_post(work, 0, &post("@lektor", "Hinweis: Eiche statt Kiefer.")).await.unwrap();
    let other = store.default_work().await.unwrap();
    assert_ne!(other, work, "das Beispielwerk ist ein anderes Werk");
    let finding =
        store.finding_insert(&new(work, keys[0], answer, 0, FindingKind::Hint, "Eiche statt Kiefer.")).await.unwrap();
    let err = store.finding_get(other, finding.id).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownFinding { work: w, id } if w == other && id == finding.id), "{err:?}");

    let long = "x".repeat(baruch_store::MAX_FINDING_CHARS + 1);
    for (bad, why) in [
        (new(work, keys[0], answer, 1, FindingKind::Hint, "  "), "leerer Text"),
        (new(work, keys[0], answer, 1, FindingKind::Hint, &long), "Text zu lang"),
        (NewFinding { persona: "", ..new(work, keys[0], answer, 1, FindingKind::Hint, "a") }, "Persona leer"),
        (
            NewFinding { thread_post: Some(1), ..new(work, keys[0], answer, 1, FindingKind::Hint, "a") },
            "zwei Herkünfte",
        ),
        (
            NewFinding { key: Some(999_999), ..new(work, keys[0], answer, 1, FindingKind::Hint, "a") },
            "fremder Abschnitt",
        ),
    ] {
        let err = store.finding_insert(&bad).await.unwrap_err();
        assert!(matches!(err, StoreError::Finding(_)), "{why}: StoreError::Finding erwartet, war {err:?}");
    }
    let err = store.finding_insert(&new(other, keys[0], answer, 1, FindingKind::Hint, "a")).await.unwrap_err();
    assert!(
        matches!(err, StoreError::Finding(_) | StoreError::UnknownPost { .. }),
        "Herkunft aus einem fremden Werk: {err:?}"
    );
    assert!(
        store.finding_insert(&new(work, keys[0], answer, 0, FindingKind::Hint, "doppelt")).await.is_err(),
        "derselbe Beitrag mit derselben Stelle zweimal (eindeutiger Index, Zerlegen bleibt so wiederholbar)"
    );
}

/// Ein Befund im ersten Abschnitt aus einem eigenen Beitrag des Lektors.
async fn one(store: &Store, work: WorkId, key: i64, body: &str) -> baruch_store::Finding {
    let answer = store.add_post(work, 0, &post("@lektor", body)).await.unwrap();
    store.finding_insert(&new(work, key, answer, 0, FindingKind::Hint, body)).await.unwrap()
}

/// Zeilen der Befundtabellen (für „nichts geändert“).
async fn rows(store: &Store) -> (i64, i64, String) {
    let findings: i64 = sqlx::query_scalar("SELECT count(*) FROM finding").fetch_one(store.pool()).await.unwrap();
    let entries: i64 = sqlx::query_scalar("SELECT count(*) FROM finding_status").fetch_one(store.pool()).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT group_concat(status || ':' || reason, ';') FROM finding")
        .fetch_one(store.pool())
        .await
        .unwrap();
    (findings, entries, state)
}

#[tokio::test]
async fn only_valid_status_changes_are_written_with_one_human_history_entry() {
    let (_dir, store, work, keys) = harbour().await;
    let finding = one(&store, work, keys[0], "Hinweis: mehr Möwen.").await;
    let unchanged = rows(&store).await;
    let idea = store
        .idea_create(&baruch_store::NewIdea {
            work: Some(work),
            key: None,
            words: &["Möwe".to_owned()],
            note: "später",
            quote: "",
            source: "",
            bible: "",
            origin: baruch_store::IdeaOrigin::Human { name: "Ida".into() },
        })
        .await
        .unwrap();
    for (change, why) in [
        (StatusChange::Rejected { reason: "  ".into() }, "verworfen ohne Begründung"),
        (StatusChange::Adopted { rev: 9 }, "übernommen mit einer Fassung, die es im Abschnitt nicht gibt"),
        (
            StatusChange::Deferred { target: DeferTarget::Node { key: keys[0] }, reason: String::new() },
            "eigener Abschnitt",
        ),
        (
            StatusChange::Deferred { target: DeferTarget::Node { key: 999_999 }, reason: String::new() },
            "fremder Abschnitt",
        ),
        (
            StatusChange::Deferred { target: DeferTarget::Idea { id: 999_999 }, reason: String::new() },
            "fremder Einfall",
        ),
        (StatusChange::Done { reason: "x".repeat(baruch_store::MAX_REASON_CHARS + 1) }, "Begründung zu lang"),
        (StatusChange::Open, "hat den Status schon"),
    ] {
        let err = store.finding_set_status(work, finding.id, &change, "Ida").await.unwrap_err();
        assert!(matches!(err, StoreError::Finding(_)), "{why}: StoreError::Finding (RPC -32602), war {err:?}");
        assert_eq!(rows(&store).await, unchanged, "{why}: nichts geändert");
    }
    // Gültig: verworfen mit Begründung → Befund und genau ein Verlaufseintrag des Menschen.
    let (before, after) = store
        .finding_set_status(work, finding.id, &StatusChange::Rejected { reason: " Absicht. ".into() }, "Ida")
        .await
        .unwrap();
    assert_eq!((before.status, after.status), (FindingStatus::Open, FindingStatus::Rejected), "vorher, nachher");
    assert_eq!(after.reason, "Absicht.", "Begründung getrimmt gespeichert");
    let history = store.finding_history(work, finding.id).await.unwrap();
    assert_eq!(history.len(), 1, "genau ein Eintrag");
    assert!(!history[0].proposed && history[0].decision.is_none(), "eine Änderung des Menschen, kein Vorschlag");
    assert_eq!(history[0].actor, baruch_store::IdeaOrigin::Human { name: "Ida".into() }, "Herkunft Mensch");
    // Übernommen in Fassung 1 (gibt es) und verschoben in einen Einfall desselben Werks.
    let (_, adopted) =
        store.finding_set_status(work, finding.id, &StatusChange::Adopted { rev: 1 }, "Ida").await.unwrap();
    assert_eq!(adopted.adopted_rev, Some(1), "Fassung über ihre uuid aufgelöst");
    assert_eq!(store.findings_adopted_in(work, keys[0]).await.unwrap(), vec![(1, vec![finding.id])], "Fassungsbaum");
    let target = DeferTarget::Idea { id: idea.id };
    let (_, deferred) = store
        .finding_set_status(
            work,
            finding.id,
            &StatusChange::Deferred { target: target.clone(), reason: String::new() },
            "Ida",
        )
        .await
        .unwrap();
    assert_eq!(deferred.deferred, Some(target), "Ziel aufgelöst");
    assert_eq!(deferred.adopted_revision, None, "die Fassung von vorher fällt weg");
    assert_eq!(store.finding_history(work, finding.id).await.unwrap().len(), 3, "je Änderung ein Eintrag");
    let counts = store.finding_open_counts(work).await.unwrap();
    assert_eq!(counts.get(&keys[0]), None, "verschoben zählt nicht als offen");
}

#[tokio::test]
async fn assistants_only_propose_and_the_human_confirms_rejects_or_finds_it_stale() {
    let (_dir, store, work, keys) = harbour().await;
    let finding = one(&store, work, keys[0], "Hinweis: Kai heller.").await;
    let lektor = baruch_store::IdeaOrigin::Persona { handle: "@lektor".into() };
    let unchanged = rows(&store).await;
    for (change, why) in [
        (StatusChange::Rejected { reason: "nein".into() }, "verwerfen darf nur der Mensch"),
        (StatusChange::Open, "wieder öffnen darf nur der Mensch"),
    ] {
        let err = store.finding_propose_status(work, finding.id, &change, &lektor, "").await.unwrap_err();
        assert!(
            matches!(&err, StoreError::Finding(m) if m.starts_with("Nur der Mensch kann")),
            "{why}: Vertrag §1.2, war {err:?}"
        );
    }
    let human = baruch_store::IdeaOrigin::Human { name: "Ida".into() };
    let done = StatusChange::Done { reason: String::new() };
    assert!(
        store.finding_propose_status(work, finding.id, &done, &human, "").await.is_err(),
        "der Mensch setzt direkt, er stellt keinen Vorschlag"
    );
    assert_eq!(rows(&store).await, unchanged, "abgelehnte Vorschläge ändern nichts");

    let entry = store.finding_propose_status(work, finding.id, &done, &lektor, "Hat sich erledigt.").await.unwrap();
    assert!(entry.proposed && entry.decision.is_none(), "offener Vorschlag");
    let pending = store.finding_get(work, finding.id).await.unwrap();
    assert_eq!(pending.status, FindingStatus::Open, "ein Vorschlag ändert den Befund nicht");
    assert_eq!(pending.pending.as_ref().map(|p| p.id), Some(entry.id), "der Befund zeigt den offenen Vorschlag");
    let twin = store.finding_propose_status(work, finding.id, &done, &lektor, "nochmal").await.unwrap_err();
    assert!(matches!(twin, StoreError::Finding(_)), "gleicher offener Vorschlag nicht doppelt: {twin:?}");
    let (confirmed, decided) = store.finding_decide(work, entry.id, true, "Ida").await.unwrap();
    assert_eq!(confirmed.status, FindingStatus::Done, "bestätigt: der Befund bekommt den Status");
    assert_eq!(decided.decision.as_deref(), Some("confirmed"), "Entscheidung vermerkt");
    assert_eq!(decided.decided_by.as_deref(), Some("Ida"), "wer entschied");
    assert!(confirmed.pending.is_none(), "kein offener Vorschlag mehr");
    assert!(store.finding_decide(work, entry.id, true, "Ida").await.is_err(), "zweimal entscheiden geht nicht");

    // Abgelehnt: der Status bleibt.
    let second = one(&store, work, keys[1], "Hinweis: Werft größer.").await;
    let proposal =
        store.finding_propose_status(work, second.id, &StatusChange::Adopted { rev: 1 }, &lektor, "").await.unwrap();
    let (kept, entry) = store.finding_decide(work, proposal.id, false, "Ida").await.unwrap();
    assert_eq!((kept.status, entry.decision.as_deref()), (FindingStatus::Open, Some("rejected")), "abgelehnt");

    // Veraltet: Der Mensch hat inzwischen selbst entschieden.
    let mcp = baruch_store::IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let third =
        store.finding_propose_status(work, second.id, &StatusChange::Adopted { rev: 1 }, &mcp, "").await.unwrap();
    store
        .finding_set_status(work, second.id, &StatusChange::Rejected { reason: "passt nicht".into() }, "Ida")
        .await
        .unwrap();
    let (kept, entry) = store.finding_decide(work, third.id, true, "Ida").await.unwrap();
    assert_eq!(entry.decision.as_deref(), Some("stale"), "inzwischen geändert → veraltet");
    assert_eq!(kept.status, FindingStatus::Rejected, "ein veralteter Vorschlag ändert nichts");
}

#[tokio::test]
async fn the_mirror_file_lists_findings_only_when_there_are_some_and_stays_deterministic() {
    let (_dir, store, work, keys) = harbour().await;
    let files = store.mirror_snapshot(work).await.unwrap();
    assert!(!files.iter().any(|f| f.path == "befunde.md"), "ohne Befunde keine befunde.md");
    let first = one(&store, work, keys[1], "Hinweis: Teer riecht.").await;
    let second = one(&store, work, keys[0], "Hinweis: Möwen kreischen.").await;
    store
        .finding_set_status(work, first.id, &StatusChange::Rejected { reason: "So soll es riechen.".into() }, "Ida")
        .await
        .unwrap();
    store.finding_set_status(work, second.id, &StatusChange::Adopted { rev: 1 }, "Ida").await.unwrap();
    let file = |files: Vec<baruch_store::MirrorFile>| {
        files.into_iter().find(|f| f.path == "befunde.md").map(|f| f.contents).expect("befunde.md")
    };
    let text = file(store.mirror_snapshot(work).await.unwrap());
    assert_eq!(text, file(store.mirror_snapshot(work).await.unwrap()), "deterministisch");
    let hafen = text.find("Der Hafen").expect("Abschnitt Der Hafen");
    let werft = text.find("Die Werft").expect("Abschnitt Die Werft");
    assert!(hafen < werft, "Baumreihenfolge, nicht Anlage: {text}");
    for needle in [
        format!("### [B{}] Hinweis · verworfen · @lektor", first.id).as_str(),
        "- Begründung: So soll es riechen.",
        "- Übernommen in Fassung 1",
        "setzt verworfen – So soll es riechen.",
    ] {
        assert!(text.contains(needle), "{needle} in befunde.md:\n{text}");
    }
}

#[tokio::test]
async fn deleting_a_finding_or_its_work_leaves_tombstones_with_the_work_uuid() {
    let (_dir, store, work, keys) = harbour().await;
    let finding = one(&store, work, keys[0], "Hinweis: Mole länger.").await;
    store.finding_set_status(work, finding.id, &StatusChange::Done { reason: String::new() }, "Ida").await.unwrap();
    let entry = store.finding_history(work, finding.id).await.unwrap().remove(0);
    let work_uuid = store.work_uuid(work).await.unwrap();
    store.post_delete(work, finding.post.unwrap()).await.unwrap();
    let still = store.finding_get(work, finding.id).await.unwrap();
    assert_eq!(still.post, None, "Beitrag gelöscht: der Befund bleibt (ON DELETE SET NULL)");
    sqlx::query("DELETE FROM finding WHERE id = ?").bind(finding.id).execute(store.pool()).await.unwrap();
    let stones: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE kind IN ('finding', 'finding_status')")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert!(stones.contains(&(finding.uuid.clone(), "finding".into(), Some(work_uuid.clone()))), "{stones:?}");
    assert!(stones.contains(&(entry.uuid.clone(), "finding_status".into(), Some(work_uuid.clone()))), "{stones:?}");
    let dumped = store.exchange_dump(work).await.unwrap();
    assert!(dumped.findings.is_empty(), "keine Befunde mehr in der Ausgabe");
    for uuid in [&finding.uuid, &entry.uuid] {
        assert!(dumped.deleted.iter().any(|t| &t.uuid == uuid), "Grabstein {uuid} in der Ausgabe (Unterversion 4)");
    }
}

/// Runde 22: `revision.amend_seq` per SQL setzen (unabhängig von der Aufgabe fassungen, die das Fortschreiben baut).
async fn set_amend(store: &Store, key: i64, rev: u32, amend: i64) {
    sqlx::query("UPDATE revision SET amend_seq = ? WHERE node_id = ? AND number = ?")
        .bind(amend)
        .bind(key)
        .bind(i64::from(rev))
        .execute(store.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn adopting_a_finding_remembers_the_stand_of_the_amended_version() {
    let (_dir, store, work, keys) = harbour().await;
    let answer = store.add_post(work, 0, &post("@lektor", "Kritisch: „Kai“ steht allein.")).await.unwrap();
    let finding = store
        .finding_insert(&new(work, keys[0], answer, 0, FindingKind::Critical, "„Kai“ steht allein."))
        .await
        .unwrap();
    assert_eq!(finding.adopted_amend, None, "ein neuer Befund kennt keinen Stand");
    set_amend(&store, keys[0], 1, 2).await;
    let (_, after) =
        store.finding_set_status(work, finding.id, &StatusChange::Adopted { rev: 1 }, "Ida").await.unwrap();
    assert_eq!(
        (after.adopted_rev, after.adopted_amend),
        (Some(1), Some(2)),
        "übernommen in Fassung 1 merkt den Stand 2 dieser Fassung zum Zeitpunkt des Setzens"
    );
    let history = store.finding_history(work, finding.id).await.unwrap();
    assert_eq!(history.last().unwrap().adopted_amend, Some(2), "der Eintrag im Verlauf trägt denselben Stand");
    // Ein späteres Fortschreiben ändert den gemerkten Stand nicht.
    set_amend(&store, keys[0], 1, 5).await;
    assert_eq!(
        store.finding_get(work, finding.id).await.unwrap().adopted_amend,
        Some(2),
        "der Stand bleibt der beim Übernehmen, nicht der jetzige"
    );
    // Ein anderer Status löscht den Stand am Befund (der Verlauf behält ihn).
    let (_, done) =
        store.finding_set_status(work, finding.id, &StatusChange::Done { reason: String::new() }, "Ida").await.unwrap();
    assert_eq!(done.adopted_amend, None, "erledigt hat keinen Stand einer Fassung");
    let history = store.finding_history(work, finding.id).await.unwrap();
    assert_eq!(history[0].adopted_amend, Some(2), "der alte Eintrag im Verlauf bleibt, wie er war");
    assert_eq!(history[1].adopted_amend, None, "der Eintrag „erledigt“ trägt keinen Stand");
}

#[tokio::test]
async fn a_confirmed_proposal_takes_the_stand_at_confirmation() {
    let (_dir, store, work, keys) = harbour().await;
    let answer = store.add_post(work, 0, &post("@lektor", "Hinweis: Die Möwen fehlen.")).await.unwrap();
    let finding =
        store.finding_insert(&new(work, keys[0], answer, 0, FindingKind::Hint, "Die Möwen fehlen.")).await.unwrap();
    let lektor = baruch_store::IdeaOrigin::Persona { handle: "@lektor".into() };
    set_amend(&store, keys[0], 1, 1).await;
    let entry = store
        .finding_propose_status(work, finding.id, &StatusChange::Adopted { rev: 1 }, &lektor, "eingeflossen")
        .await
        .unwrap();
    assert_eq!(entry.adopted_amend, Some(1), "der Vorschlag merkt den Stand, den die Persona sah");
    assert_eq!(
        store.finding_get(work, finding.id).await.unwrap().adopted_amend,
        None,
        "der Befund selbst ändert sich durch einen Vorschlag nicht"
    );
    set_amend(&store, keys[0], 1, 3).await;
    let (after, decided) = store.finding_decide(work, entry.id, true, "Ida").await.unwrap();
    assert_eq!(
        (after.status, after.adopted_rev, after.adopted_amend),
        (FindingStatus::Adopted, Some(1), Some(3)),
        "beim Bestätigen zählt der Stand der Fassung in diesem Augenblick"
    );
    assert_eq!(decided.adopted_amend, Some(3), "der bestätigte Eintrag trägt denselben Stand wie der Befund");
    let mirror = store.mirror_snapshot(work).await.unwrap();
    let befunde = mirror.iter().find(|f| f.path == "befunde.md").expect("befunde.md im Spiegel");
    assert!(
        befunde.contents.contains("- Übernommen in Fassung 1 · Stand 3"),
        "der Spiegel nennt Fassung und Stand: {}",
        befunde.contents
    );
}

#[tokio::test]
async fn a_never_amended_version_shows_only_its_number() {
    let (_dir, store, work, keys) = harbour().await;
    let answer = store.add_post(work, 0, &post("@lektor", "Hinweis: Die Werft fehlt.")).await.unwrap();
    let finding =
        store.finding_insert(&new(work, keys[0], answer, 0, FindingKind::Hint, "Die Werft fehlt.")).await.unwrap();
    let (_, after) =
        store.finding_set_status(work, finding.id, &StatusChange::Adopted { rev: 1 }, "Ida").await.unwrap();
    assert_eq!(after.adopted_amend, Some(0), "Stand 0 = nie fortgeschrieben (gemerkt, aber ohne Zusatz gezeigt)");
    assert_eq!(baruch_store::findings::adopted_words(1, after.adopted_amend), "Fassung 1", "ohne „Stand 0“");
    let mirror = store.mirror_snapshot(work).await.unwrap();
    let befunde = mirror.iter().find(|f| f.path == "befunde.md").unwrap();
    assert!(befunde.contents.contains("- Übernommen in Fassung 1\n"), "wie vor Runde 22: {}", befunde.contents);
}
