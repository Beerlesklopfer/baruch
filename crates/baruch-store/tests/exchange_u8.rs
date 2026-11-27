// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (spiegel-entwurf, contract §2.10/§2.11): exchange format sub-version 8 in the store – amendments
//! including state and state of the base, draft in the typesetting settings, state on findings (read anew and write
//! back), fast-forwarding known revisions when merging and the new part as a request with identifier and revisions
//! (agree, refuse, merge again). Invented sample work from `baruch_exchange::sample`; real SQLite file in the temp
//! folder; every assertion with a reason.

use baruch_exchange::sample::{SAMPLE_TRICKY_BODY, sample_document, sample_document_u8, sample_uuid};
use baruch_exchange::{
    ExchangeAmend, ExchangeAmendKind, ExchangeAuthor, ExchangeDocument, ExchangeNode, ExchangeNodeKind,
    ExchangeRevision, ExchangeRevisionStatus, ExchangeTombstoneKind,
};
use baruch_store::exchange_apply::{ApplyMode, DeviationAction};
use baruch_store::{OutlineOp, Store, WorkId};

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000022";

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn import_new(store: &Store, doc: &ExchangeDocument) -> WorkId {
    store.exchange_apply(doc, SHA, ApplyMode::New, None).await.unwrap().work
}

fn find<'a>(nodes: &'a mut [ExchangeNode], uuid: &str) -> Option<&'a mut ExchangeNode> {
    for node in nodes {
        if node.uuid == uuid {
            return Some(node);
        }
        if let Some(found) = find(&mut node.children, uuid) {
            return Some(found);
        }
    }
    None
}

/// The amended revision „Der Eimer“, revision 1 (identifier 111) of the sample.
fn amended(doc: &mut ExchangeDocument) -> &mut ExchangeRevision {
    let bucket = find(&mut doc.work.outline, &sample_uuid(11)).expect("„Der Eimer“");
    bucket.revisions.iter_mut().find(|r| r.uuid == sample_uuid(111)).expect("Fassung 1")
}

/// Without keys (they only hold in their instance).
fn without_keys(nodes: &[ExchangeNode]) -> Vec<ExchangeNode> {
    nodes.iter().map(|n| ExchangeNode { key: None, children: without_keys(&n.children), ..n.clone() }).collect()
}

async fn amend_rows(store: &Store, revision_uuid: &str) -> Vec<(i64, String, String)> {
    sqlx::query_as(
        "SELECT a.seq, a.kind, a.uuid FROM revision_amend a JOIN revision r ON r.id = a.revision_id \
         WHERE r.uuid = ? ORDER BY a.seq",
    )
    .bind(revision_uuid)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn a_new_import_carries_amends_stands_draft_and_adopted_amend_and_dumps_them_back_exactly() {
    let (_dir, store) = open().await;
    let doc = sample_document_u8();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    assert_eq!(applied.report.add.amends, 3, "drei Fortschreibungen kommen mit");
    let work = applied.work;
    let (seq, based): (i64, Option<i64>) =
        sqlx::query_as("SELECT amend_seq, based_on_amend FROM revision WHERE uuid = ?")
            .bind(sample_uuid(111))
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!((seq, based), (3, None), "Stand 3 an der fortgeschriebenen Fassung");
    let based: Option<i64> = sqlx::query_scalar("SELECT based_on_amend FROM revision WHERE uuid = ?")
        .bind(sample_uuid(113))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(based, Some(2), "der Vorschlag beruht auf Stand 2 seiner Basis");
    let rows = amend_rows(&store, &sample_uuid(111)).await;
    assert_eq!(
        rows,
        vec![
            (1, "amend".to_owned(), sample_uuid(2200)),
            (2, "undo".to_owned(), sample_uuid(2201)),
            (3, "amend".to_owned(), sample_uuid(2202)),
        ],
        "jeder Schritt mit seiner Kennung aus der Datei"
    );
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(without_keys(&dumped.outline), without_keys(&doc.work.outline), "Fassungen samt Verlauf zurück");
    assert_eq!(dumped.findings, doc.work.findings, "Stand am Befund und am Verlaufseintrag zurück");
    assert_eq!(dumped.typeset, doc.work.typeset, "Entwurf in den Satz-Einstellungen zurück");
    assert!(
        dumped.deleted.iter().any(|t| t.kind == ExchangeTombstoneKind::Amend && t.uuid == sample_uuid(2210)),
        "Grabstein `fortschreibung` zurück: {:?}",
        dumped.deleted
    );
    // Byte-identical in the XML: the same keys inserted, then the same work as in the file.
    let mut again = doc.clone();
    again.work.outline = dumped.outline.clone();
    fn keys_from(target: &mut [ExchangeNode], source: &[ExchangeNode]) {
        for (t, s) in target.iter_mut().zip(source) {
            t.key = s.key;
            keys_from(&mut t.children, &s.children);
        }
    }
    keys_from(&mut again.work.outline, &doc.work.outline);
    again.work.findings = dumped.findings.clone();
    again.work.typeset = dumped.typeset.clone();
    let xml =
        |d: &ExchangeDocument| baruch_exchange::write::write_xml(d, baruch_exchange::write::ImageMode::Inline).unwrap();
    assert_eq!(xml(&again), xml(&doc), "Fortschreibungen, Stände und Entwurf byte-gleich im XML");
}

/// File in which „Der Eimer“ revision 1 has only one step so far (text = after step 1).
fn after_first_step() -> ExchangeDocument {
    let mut doc = sample_document_u8();
    let revision = amended(&mut doc);
    revision.amends.truncate(1);
    revision.amend_seq = 1;
    revision.body = revision.amends[0].after.clone();
    doc
}

#[tokio::test]
async fn a_known_revision_whose_amends_are_a_prefix_is_fast_forwarded_and_a_second_merge_adds_nothing() {
    let (_dir, store) = open().await;
    let work = import_new(&store, &after_first_step()).await;
    let revisions_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM revision").fetch_one(store.pool()).await.unwrap();
    let full = sample_document_u8();
    let applied = store.exchange_apply(&full, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.report.add.amends, 2, "die zwei fehlenden Schritte kommen dazu: {:?}", applied.report.items);
    assert_eq!(applied.report.add.revisions, 0, "keine abweichende Fassung beim Vorspulen");
    let (body, seq): (String, i64) = sqlx::query_as("SELECT body, amend_seq FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!((body.as_str(), seq), (SAMPLE_TRICKY_BODY, 3), "Text und Stand folgen der Datei");
    assert_eq!(amend_rows(&store, &sample_uuid(111)).await.len(), 3, "Verlauf vollständig");
    let revisions_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM revision").fetch_one(store.pool()).await.unwrap();
    assert_eq!(revisions_after, revisions_before, "keine neue Fassung");
    let hint = applied.report.deviations.iter().find(|d| d.uuid == sample_uuid(111)).expect("Vorspulen gemeldet");
    assert_eq!(hint.action, DeviationAction::Hinweis, "als Hinweis gemeldet");
    assert!(hint.label.contains("vorgespult von Stand 1 auf Stand 3"), "Meldung nennt die Stände: {}", hint.label);
    let again = store.exchange_apply(&full, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.total(), 0, "ein zweiter Abgleich fügt nichts hinzu: {:?}", again.report.items);
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(without_keys(&dumped.outline), without_keys(&full.work.outline), "jetzt gleich der Datei");
}

#[tokio::test]
async fn amends_that_are_no_prefix_give_a_divergent_revision_as_before() {
    let (_dir, store) = open().await;
    let mut local = after_first_step();
    // Here the amending went differently: the same text, but a step with a different identifier.
    amended(&mut local).amends[0].uuid = sample_uuid(2299);
    import_new(&store, &local).await;
    let applied = store.exchange_apply(&sample_document_u8(), SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.report.add.amends, 0, "nichts vorgespult");
    assert_eq!(applied.report.add.revisions, 1, "abweichende Fassung wie bisher: {:?}", applied.report.items);
    assert!(
        applied.report.deviations.iter().any(|d| d.action == DeviationAction::Fassung),
        "als weitere Fassung gemeldet"
    );
    let seq: i64 = sqlx::query_scalar("SELECT amend_seq FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(seq, 1, "die Fassung hier bleibt, wie sie war");
}

/// After a merge that must not fast-forward: revision 111 unchanged here (text after step 1, state 1, one step in
/// the history), no amendment added, the text of the file comes as a deviating revision.
async fn assert_not_fast_forwarded(store: &Store, applied: &baruch_store::exchange_apply::ExchangeApplied, why: &str) {
    let first_after = amended(&mut after_first_step()).amends[0].after.clone();
    assert_eq!(applied.report.add.amends, 0, "{why}: keine Fortschreibung übernommen: {:?}", applied.report.items);
    assert_eq!(
        applied.report.add.revisions, 1,
        "{why}: abweichende Fassung wie vor Runde 22: {:?}",
        applied.report.items
    );
    let (body, seq): (String, i64) = sqlx::query_as("SELECT body, amend_seq FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!((body, seq), (first_after, 1), "{why}: die Fassung hier bleibt, wie sie war (nicht überschrieben)");
    assert_eq!(amend_rows(store, &sample_uuid(111)).await.len(), 1, "{why}: Verlauf hier unverändert");
}

#[tokio::test]
async fn a_persona_revision_is_never_fast_forwarded_in_place() {
    // Finding (follow-up fix R22): fast-forwarding rewrote every known revision. A revision of a persona (here an
    // accepted, current one) must, as locally (`NotOwn`), never be amended in place.
    let persona = ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "probe/modell-7b".into() };
    let (_dir, store) = open().await;
    let mut local = after_first_step();
    amended(&mut local).author = persona.clone();
    import_new(&store, &local).await;
    let mut full = sample_document_u8();
    amended(&mut full).author = persona;
    let applied = store.exchange_apply(&full, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_not_fast_forwarded(&store, &applied, "Persona-Fassung").await;
}

#[tokio::test]
async fn a_revision_that_is_assistant_here_is_not_fast_forwarded_even_if_the_file_says_human() {
    // The file claims "human", here the revision comes from an MCP client: decisive is the origin here.
    let (_dir, store) = open().await;
    import_new(&store, &after_first_step()).await;
    // The origin is immutable by trigger; the test takes it out briefly and puts it back verbatim (like
    // `age_revision` in revision_amend.rs), to recreate "here by an MCP client".
    let trigger: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'revision_origin_immutable'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let mut conn = store.pool().acquire().await.unwrap();
    sqlx::query("DROP TRIGGER revision_origin_immutable").execute(&mut *conn).await.unwrap();
    sqlx::query(
        "UPDATE revision SET author_kind = 'assistant', author_name = '@mcp:probe', model = 'probe' WHERE uuid = ?",
    )
    .bind(sample_uuid(111))
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(trigger.as_str())).execute(&mut *conn).await.unwrap();
    drop(conn);
    let applied = store.exchange_apply(&sample_document_u8(), SHA, ApplyMode::Merge, None).await.unwrap();
    assert_not_fast_forwarded(&store, &applied, "MCP-Fassung hier").await;
    let kind: String = sqlx::query_scalar("SELECT author_kind FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(kind, "assistant", "Herkunft hier bleibt");
}

#[tokio::test]
async fn a_revision_that_is_superseded_here_is_not_fast_forwarded() {
    // Here revision 2 is meanwhile current, revision 1 superseded (status swap directly in the database, as
    // switching leaves it). Nobody amends a superseded revision locally either.
    let (_dir, store) = open().await;
    import_new(&store, &after_first_step()).await;
    for (uuid, status) in [(sample_uuid(111), "superseded"), (sample_uuid(112), "current")] {
        sqlx::query("UPDATE revision SET status = ? WHERE uuid = ?")
            .bind(status)
            .bind(uuid)
            .execute(store.pool())
            .await
            .unwrap();
    }
    let applied = store.exchange_apply(&sample_document_u8(), SHA, ApplyMode::Merge, None).await.unwrap();
    assert_not_fast_forwarded(&store, &applied, "ersetzte Fassung").await;
}

#[tokio::test]
async fn amends_that_do_not_reproduce_the_text_of_the_file_are_not_fast_forwarded() {
    // Finding (follow-up fix R22): the steps of the file must reproduce the change without gaps. Two cases:
    // (a) the text of the revision is an entirely different one than the "after" of the last step – fast-forwarded,
    //     the history would show harmless steps while the text would be replaced entirely;
    // (b) a step does not start where the previous one ends – an undo could otherwise reset to a text that never
    //     existed here.
    let (_dir, store) = open().await;
    import_new(&store, &after_first_step()).await;
    let mut replaced = sample_document_u8();
    amended(&mut replaced).body = "== Der Eimer\n\nGanz anderer, untergeschobener Text.\n".into();
    let applied = store.exchange_apply(&replaced, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_not_fast_forwarded(&store, &applied, "Text ≠ letzter Schritt").await;

    let (_dir, store) = open().await;
    import_new(&store, &after_first_step()).await;
    let mut gap = sample_document_u8();
    amended(&mut gap).amends[1].before = "== Der Eimer\n\nNie gesehener Zwischenstand.\n".into();
    let applied = store.exchange_apply(&gap, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_not_fast_forwarded(&store, &applied, "Lücke im Verlauf").await;
}

/// Work with two chapters; the other side puts a part (700) with two revisions (701 amended, 702 proposal) around both.
fn part_documents() -> (ExchangeDocument, ExchangeDocument) {
    let mut doc = sample_document();
    let bucket = find(&mut doc.work.outline, &sample_uuid(11)).unwrap().clone();
    let rope = find(&mut doc.work.outline, &sample_uuid(12)).unwrap().clone();
    doc.work.outline = vec![bucket, rope];
    doc.work.outline[0].posts[1].refs.clear();
    let local = doc.clone();
    let heading = "#heading(level: 1, numbering: none)[Ganz neuer Teil] <baruch-part>\n";
    let first = ExchangeRevision {
        uuid: sample_uuid(701),
        number: 1,
        status: ExchangeRevisionStatus::Current,
        author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
        body: format!("{heading}\nVorwort zum Teil.\n"),
        created: "2026-02-04T09:00:00.000Z".into(),
        amend_seq: 1,
        amends: vec![ExchangeAmend {
            uuid: sample_uuid(705),
            seq: 1,
            kind: ExchangeAmendKind::Amend,
            undo_of: None,
            author: "Hanna Probe".into(),
            created: "2026-02-04T10:00:00.000Z".into(),
            before: heading.to_owned(),
            after: format!("{heading}\nVorwort zum Teil.\n"),
        }],
        ..ExchangeRevision::default()
    };
    let second = ExchangeRevision {
        uuid: sample_uuid(702),
        number: 2,
        status: ExchangeRevisionStatus::Proposed,
        author: ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "probe/modell-7b".into() },
        based_on: Some(sample_uuid(701)),
        based_on_amend: Some(1),
        body: format!("{heading}\nKürzeres Vorwort.\n"),
        created: "2026-02-05T09:00:00.000Z".into(),
        ..ExchangeRevision::default()
    };
    let chapters = std::mem::take(&mut doc.work.outline);
    doc.work.outline = vec![ExchangeNode {
        uuid: sample_uuid(700),
        kind: ExchangeNodeKind::Part,
        numbered: true,
        title: "Ganz neuer Teil".into(),
        revisions: vec![first, second],
        children: chapters,
        ..ExchangeNode::default()
    }];
    (local, doc)
}

async fn tombstones(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone").fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn agreeing_to_a_new_part_creates_it_with_its_uuid_and_revisions_and_a_second_merge_reports_nothing() {
    let (_dir, store) = open().await;
    let (local, there) = part_documents();
    let work = import_new(&store, &local).await;
    let applied = store.exchange_apply(&there, SHA, ApplyMode::Merge, None).await.unwrap();
    let requests = store.outline_requests(work, true).await.unwrap();
    assert_eq!(requests.len(), 1, "genau ein Antrag: {requests:?}");
    let request = &requests[0];
    let OutlineOp::Add { part: Some(part), .. } = &request.op else {
        panic!("Antrag Add mit Kennung: {:?}", request.op)
    };
    assert_eq!(part.uuid, sample_uuid(700), "Kennung des Teils im Antrag");
    assert_eq!(part.revisions.len(), 2, "beide Fassungen im Antrag");
    assert!(request.summary.contains("mit Kennung und 2 Fassungen"), "Anzeige nennt es: {}", request.summary);
    assert!(applied.report.requests.iter().any(|r| r.op == "add"), "Vorschau nennt den Antrag");
    let stones_before = tombstones(&store).await;

    let decided = store.outline_request_decide(work, request.id, true, "Hanna Probe").await.unwrap();
    assert_eq!(decided.request.decision, Some(baruch_store::Decision::Confirmed), "{:?}", decided.request.message);
    assert!(decided.change.is_some(), "Zustimmen ändert die Gliederung (ein Spiegel-Commit beim Server)");
    let (key, parent): (i64, Option<i64>) =
        sqlx::query_as("SELECT id, parent_seq FROM node WHERE uuid = ? AND work_id = ?")
            .bind(sample_uuid(700))
            .bind(work)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(parent, None, "der Teil steht oben");
    type RevisionRow = (i64, String, String, String, i64, Option<i64>, Option<i64>);
    let revisions: Vec<RevisionRow> = sqlx::query_as(
        "SELECT number, uuid, status, author_kind, amend_seq, based_on_amend, based_on FROM revision \
         WHERE node_id = ? ORDER BY number",
    )
    .bind(key)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        revisions,
        vec![
            (1, sample_uuid(701), "current".into(), "human".into(), 1, None, None),
            (2, sample_uuid(702), "proposed".into(), "assistant".into(), 0, Some(1), Some(1)),
        ],
        "genau die Fassungen der Datei mit Nummer, Kennung, Status, Herkunft, Stand und Basis"
    );
    assert_eq!(amend_rows(&store, &sample_uuid(701)).await, vec![(1, "amend".into(), sample_uuid(705))], "Verlauf");
    assert_eq!(tombstones(&store).await, stones_before, "die ersetzte Fassung 1 hinterlässt keinen Grabstein");
    let children: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM node c JOIN node p ON p.seq = c.parent_seq AND p.work_id = c.work_id \
         WHERE p.id = ? AND c.work_id = ?",
    )
    .bind(key)
    .bind(work)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(children, 2, "beide Kapitel stehen unter dem neuen Teil");
    store.load_work(work).await.expect("das Werk lädt (genau eine aktuelle Fassung je Punkt)");

    let again = store.exchange_apply(&there, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.total(), 0, "erneuter Abgleich fügt nichts hinzu: {:?}", again.report.items);
    assert!(again.report.requests.is_empty(), "und stellt keinen Antrag: {:?}", again.report.requests);
    assert!(
        !again.report.deviations.iter().any(|d| d.action != DeviationAction::Hinweis),
        "keine Abweichung außer Hinweisen: {:?}",
        again.report.deviations
    );
}

#[tokio::test]
async fn rejecting_the_new_part_creates_nothing_and_leaves_no_tombstone() {
    let (_dir, store) = open().await;
    let (local, there) = part_documents();
    let work = import_new(&store, &local).await;
    store.exchange_apply(&there, SHA, ApplyMode::Merge, None).await.unwrap();
    let request = store.outline_requests(work, true).await.unwrap().remove(0);
    let nodes: i64 = sqlx::query_scalar("SELECT count(*) FROM node").fetch_one(store.pool()).await.unwrap();
    let stones = tombstones(&store).await;
    store.outline_request_decide(work, request.id, false, "Hanna Probe").await.unwrap();
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM node").fetch_one(store.pool()).await.unwrap();
    assert_eq!(after, nodes, "abgelehnt: kein Punkt angelegt");
    assert_eq!(tombstones(&store).await, stones, "abgelehnt: kein Grabstein");
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM revision WHERE uuid = ?")
        .bind(sample_uuid(701))
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(found, None, "keine Fassung des Teils");
}

#[tokio::test]
async fn the_revision_policy_is_not_in_the_exchange_format() {
    // Contract §2.3: the setting „Fassungen“ is the working method of the instance. A row in work_revision_policy
    // does not change the output.
    let (_dir, store) = open().await;
    let work = import_new(&store, &sample_document_u8()).await;
    let before = store.exchange_dump(work).await.unwrap();
    sqlx::query(
        "INSERT INTO work_revision_policy (work_id, amend, max_change_percent, new_after_minutes) VALUES (?, 0, 50, 0)",
    )
    .bind(work)
    .execute(store.pool())
    .await
    .unwrap();
    let after = store.exchange_dump(work).await.unwrap();
    assert_eq!(after, before, "die Einstellung „Fassungen“ steht nicht im Format");
}
