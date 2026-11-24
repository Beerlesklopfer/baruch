// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einlesen einer Austauschdatei (Runde 26, Vertrag F6): neu und abgleichen über `Store::exchange_preview` und
//! `Store::exchange_apply`, mit dem erfundenen Beispielwerk aus `baruch_exchange::sample`. Echte SQLite-Datei im
//! Temp-Ordner; jede Zusicherung mit Begründung.

use std::collections::BTreeMap;

use baruch_exchange::sample::{SAMPLE_PNG_GREEN, sample_document, sample_uuid};
use baruch_exchange::{
    ExchangeAssetRevision, ExchangeAssetStatus, ExchangeAuthor, ExchangeDocument, ExchangeIdea, ExchangeIdeaStatus,
    ExchangeNode, ExchangeNodeKind, ExchangeOriginKind, ExchangePost, ExchangeRevision, ExchangeRevisionStatus,
    sha256_hex,
};
use baruch_store::exchange_apply::{ApplyMode, DeviationAction, derive_uuid};
use baruch_store::{IdeaOrigin, OutlineOp, Store, StoreError, WorkId};

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000026";

/// Alle Tabellen mit Werkinhalt und die Anträge (Zeilenzahlen vorher = nachher bei jeder Ablehnung).
const TABLES: &[&str] = &[
    "work",
    "node",
    "revision",
    "work_template_revision",
    "post",
    "post_ref",
    "idea",
    "idea_word",
    "chat_thread",
    "chat_thread_post",
    "asset",
    "asset_path",
    "asset_revision",
    "work_book",
    "work_timing",
    "work_bible",
    "work_sword_source",
    "outline_request",
    "uuid_tombstone",
    // Gedankenkarte (Runde 25)
    "idea_link",
    "idea_map_pos",
    // Umschlag-Anordnung und Satzbild (Runde 27, Unterversion 3)
    "cover_block",
    "work_sheet",
    // Befunde und alte Beitragsfassungen (Runde 28, Unterversion 4)
    "finding",
    "finding_status",
    "post_version",
    // Satz-Einstellungen (Runde 29, Unterversion 6)
    "work_typeset",
    // Glossar und eigene Register (Runde 31, Unterversion 7)
    "glossary_term",
    "glossary_variant",
    "index_register",
];

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn counts(store: &Store) -> BTreeMap<&'static str, i64> {
    let mut out = BTreeMap::new();
    for table in TABLES {
        let n: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(store.pool()).await.unwrap();
        out.insert(*table, n);
    }
    out
}

/// Vollständiger Abzug aller Zeilen (alle Spalten außer den genannten) je Tabelle, Schlüssel rowid.
async fn dump(store: &Store, skip: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for table in TABLES {
        let columns: Vec<String> = sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(store.pool())
            .await
            .unwrap();
        let kept: Vec<String> = columns
            .into_iter()
            .filter(|c| !skip.contains(&(*table, c.as_str())))
            .map(|c| format!("quote(\"{c}\")"))
            .collect();
        let query = format!("SELECT rowid, {} FROM {table}", kept.join(" || '|' || "));
        let rows: Vec<(i64, String)> = sqlx::query_as(&query).fetch_all(store.pool()).await.unwrap();
        for (rowid, row) in rows {
            out.insert(format!("{table}#{rowid}"), row);
        }
    }
    out
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

fn node(doc: &mut ExchangeDocument, n: u32) -> &mut ExchangeNode {
    find(&mut doc.work.outline, &sample_uuid(n)).expect("Punkt im Beispiel")
}

async fn key_of(store: &Store, uuid: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE uuid = ?").bind(uuid).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn a_new_import_keeps_uuids_numbers_status_times_and_marks() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let work = applied.work;
    assert_eq!(applied.mode, ApplyMode::New, "unbekannte Werk-UUID → neu");
    assert_eq!(store.work_uuid(work).await.unwrap(), sample_uuid(1), "das Werk behält seine Kennung");
    assert_eq!(applied.title, "Der Brunnen im Hof", "Titel aus der Datei");

    // Fassungen von „Der Eimer“: Nummern, Status, Basis, Zeiten und Text exakt.
    type RevisionRow = (i64, String, Option<i64>, String, String, Option<String>, String, Option<String>);
    let rows: Vec<RevisionRow> = sqlx::query_as(
        "SELECT r.number, r.status, r.based_on, r.created, r.body, r.edited, r.uuid, r.model FROM revision r \
         JOIN node n ON n.id = r.node_id WHERE n.uuid = ? ORDER BY r.number",
    )
    .bind(sample_uuid(11))
    .fetch_all(store.pool())
    .await
    .unwrap();
    let statuses: Vec<(i64, &str)> = rows.iter().map(|r| (r.0, r.1.as_str())).collect();
    assert_eq!(
        statuses,
        vec![(1, "current"), (2, "superseded"), (3, "proposed"), (4, "rejected")],
        "Nummern und Status wie in der Datei, die aktuelle Markierung bleibt bei Fassung 1"
    );
    assert_eq!(rows[1].2, Some(1), "Basis über die UUID auf Nummer 1 abgebildet");
    assert_eq!(rows[0].3, "2026-01-05T09:00:00.000Z", "Zeit so, wie sie in der Datei steht");
    assert_eq!(rows[0].4, baruch_exchange::sample::SAMPLE_TRICKY_BODY, "Text exakt, auch Ränder, Tab und \\r");
    assert_eq!(rows[1].5.as_deref(), Some("2026-01-06T15:00:00.000Z"), "geändert-Zeit bleibt");
    assert_eq!(rows[2].6, sample_uuid(113), "Fassung behält ihre Kennung");
    assert_eq!(rows[2].7.as_deref(), Some("probe/modell-7b"), "Modell des Assistenten als Herkunft");

    // Gliederung: Teile, Kapitel, Abschnitt, „Das Seil“ ohne Nummer.
    let nodes: Vec<(String, String, i64, String)> =
        sqlx::query_as("SELECT uuid, kind, numbered, title FROM node WHERE work_id = ? ORDER BY seq")
            .bind(work)
            .fetch_all(store.pool())
            .await
            .unwrap();
    let uuids: Vec<&str> = nodes.iter().map(|n| n.0.as_str()).collect();
    let expected: Vec<String> = [10, 11, 12, 13, 20, 21].iter().map(|&n| sample_uuid(n)).collect();
    assert_eq!(uuids, expected.iter().map(String::as_str).collect::<Vec<_>>(), "Baumreihenfolge mit Kennungen");
    assert_eq!(nodes[2].2, 0, "„Das Seil“ zählt ohne Nummer");
    assert_eq!(nodes[0].1, "part", "erster Punkt ist ein Teil");

    // Vorlagenfassungen und aktueller Vorlagentext.
    let templates: Vec<(i64, String)> =
        sqlx::query_as("SELECT number, uuid FROM work_template_revision WHERE work_id = ? ORDER BY number")
            .bind(work)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(templates, vec![(1, sample_uuid(80)), (2, sample_uuid(81))], "beide Vorlagenfassungen");
    assert_eq!(store.work_template(work).await.unwrap(), doc.work.template.current, "aktueller Vorlagentext");

    // Einfälle: gelöscht bleibt gelöscht, Bezug über die UUID, erledigt mit Art.
    type IdeaRow = (String, Option<String>, String, Option<String>, Option<i64>);
    let ideas: Vec<IdeaRow> =
        sqlx::query_as("SELECT uuid, deleted, status, done_as, node_id FROM idea WHERE work_id = ? ORDER BY uuid")
            .bind(work)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(ideas.len(), 3, "drei Einfälle");
    assert_eq!(ideas[2].1.as_deref(), Some("2026-01-07T09:00:00.000Z"), "gelöschter Einfall bleibt gelöscht");
    assert_eq!((ideas[1].2.as_str(), ideas[1].3.as_deref()), ("done", Some("post")), "erledigt als Beitrag");
    assert_eq!(ideas[0].4, Some(key_of(&store, &sample_uuid(12)).await), "Bezug auf „Das Seil“");
    // done_ref mit der Kennung eines Punkts wird wieder „key <Schlüssel>“ (Gegenstück zur Ausgabe), sonst wörtlich.
    let done_ref: Option<String> = sqlx::query_scalar("SELECT done_ref FROM idea WHERE uuid = ?")
        .bind(sample_uuid(41))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(done_ref.as_deref(), Some(sample_uuid(31).as_str()), "Kennung eines Beitrags bleibt wörtlich");

    // Beiträge samt Bezug (Stelle und Fassungsnummer).
    let loaded = store.load_work(work).await.unwrap();
    let bucket = loaded.nodes.iter().find(|n| n.title == "Der Eimer").unwrap();
    assert_eq!(bucket.posts.len(), 2, "zwei Beiträge");
    assert_eq!((bucket.posts[0].refs[0].node, bucket.posts[0].refs[0].rev), (bucket.id, 1), "Bezug auf Fassung 1");
    let lamp = loaded.nodes.iter().find(|n| n.title == "Die Lampe").unwrap();
    assert_eq!((bucket.posts[1].refs[0].node, bucket.posts[1].refs[0].rev), (lamp.id, 0), "Bezug ohne Fassung → 0");

    // Faden, Bilder samt Umschlag, Grabsteine.
    let thread_posts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chat_thread_post p JOIN chat_thread t ON t.id = p.thread_id WHERE t.uuid = ?",
    )
    .bind(sample_uuid(50))
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(thread_posts, 2, "Faden mit zwei Beiträgen");
    let cover: Option<String> = sqlx::query_scalar(
        "SELECT a.uuid FROM work_book b JOIN asset a ON a.id = b.cover_asset_id WHERE b.work_id = ?",
    )
    .bind(work)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(cover, Some(sample_uuid(60)), "Umschlag über die Bild-UUID");
    let png: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT r.number, r.status, r.sha256 FROM asset_revision r JOIN asset a ON a.id = r.asset_id WHERE a.uuid = ? \
         ORDER BY r.number",
    )
    .bind(sample_uuid(60))
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(png.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), vec!["superseded", "current"], "Bildfassungen");
    assert_eq!(png[0].2, sha256_hex(SAMPLE_PNG_GREEN), "Bytes unverändert (Prüfsumme)");
    let svg: (String, String) = sqlx::query_as(
        "SELECT r.encoding, r.content FROM asset_revision r JOIN asset a ON a.id = r.asset_id WHERE a.uuid = ?",
    )
    .bind(sample_uuid(63))
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(svg, ("svg".into(), baruch_exchange::sample::SAMPLE_SVG.into()), "SVG als Text wie beim Hochladen");
    let paths: Vec<String> = sqlx::query_scalar(
        "SELECT p.path FROM asset_path p JOIN asset a ON a.id = p.asset_id WHERE a.uuid = ? ORDER BY p.path",
    )
    .bind(sample_uuid(63))
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(paths, vec!["/bilder/lampe.svg", "bilder/lampe.svg"], "beide Schreibweisen desselben Bilds bleiben");
    let tombstone: (String, Option<String>) =
        sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
            .bind(sample_uuid(70))
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(tombstone, ("node".into(), Some(sample_uuid(1))), "Grabsteinliste mitgenommen");
    let timing: (i64, Option<i64>) =
        sqlx::query_as("SELECT words_per_minute, target_seconds FROM work_timing WHERE work_id = ?")
            .bind(work)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(timing, (150, Some(1800)), "Sprechdauer");
    let sources: i64 = sqlx::query_scalar("SELECT count(*) FROM work_sword_source WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(sources, 2, "zwei SWORD-Quellen");
    assert_eq!(applied.report.add.nodes, 6, "sechs Punkte gezählt");
    assert_eq!(applied.report.add.revisions, 9, "neun Fassungen gezählt (4 + 5 × 1)");
}

#[tokio::test]
async fn a_taken_title_is_refused_and_a_title_of_ones_own_works() {
    let (_dir, store) = open().await;
    // Ein Werk hier trägt schon den Titel des Beispiels (Titelregeln wie beim Import, `title_key`).
    let new = baruch_store::NewWork {
        title: "Der Brunnen im Hof",
        author: "Anna",
        kind: baruch_core::WorkKind::Book,
        preset: baruch_store::Preset::Book,
        subtitle: "",
        audience: "",
        bible: "",
    };
    let first = store.create_work(&new, "Anna").await.unwrap().work;
    let doc = sample_document();
    let before = counts(&store).await;
    let refused = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await;
    assert!(
        matches!(refused, Err(StoreError::DuplicateTitle { work, .. }) if work == first),
        "vergebener Titel → DuplicateTitle: {refused:?}"
    );
    assert_eq!(counts(&store).await, before, "nichts geschrieben");
    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    assert!(
        preview.refusals.iter().any(|r| r.contains("Der Brunnen im Hof")),
        "Vorschau nennt den vergebenen Titel: {:?}",
        preview.refusals
    );
    assert_eq!(preview.add.nodes, 6, "die Vorschau zählt trotzdem, was käme");
    assert_eq!(counts(&store).await, before, "die Vorschau schreibt nichts");
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, Some("Der Brunnen, zweite Fassung")).await.unwrap();
    assert_eq!(applied.title, "Der Brunnen, zweite Fassung", "eigener Titel");
    assert_eq!(store.work_uuid(applied.work).await.unwrap(), sample_uuid(1), "Kennung bleibt die der Datei");
}

#[tokio::test]
async fn the_mode_must_fit_the_file() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let before = counts(&store).await;
    let merge = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await;
    assert!(matches!(merge, Err(StoreError::Refused(_))), "Abgleichen eines unbekannten Werks → Refused: {merge:?}");
    assert_eq!(counts(&store).await, before, "nichts geschrieben");
    import_new(&store, &doc).await;
    let again = counts(&store).await;
    let new = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await;
    assert!(matches!(new, Err(StoreError::Refused(_))), "Neu bei bekanntem Werk → Refused: {new:?}");
    assert_eq!(counts(&store).await, again, "nichts geschrieben");
}

#[tokio::test]
async fn merging_the_same_file_again_adds_nothing() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let work = import_new(&store, &doc).await;
    let before = counts(&store).await;
    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    assert_eq!(preview.mode, ApplyMode::Merge, "bekannte Werk-UUID → abgleichen");
    assert_eq!(preview.known_work, Some(work), "das bekannte Werk");
    assert_eq!(preview.add.total(), 0, "nichts kommt hinzu: {:?}", preview.items);
    assert!(preview.requests.is_empty(), "keine Anträge: {:?}", preview.requests);
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.report.add.total(), 0, "auch beim Übernehmen nichts");
    assert!(applied.requests.is_empty(), "keine Anträge angelegt");
    assert_eq!(counts(&store).await, before, "keine Zeile mehr, kein Doppel");
}

#[tokio::test]
async fn a_changed_text_arrives_as_a_further_revision_with_a_derived_uuid() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    let changed = "  == Der Eimer\n\n\tDer Eimer hält jetzt Regenwasser.\n";
    node(&mut doc, 11).revisions[0].body = changed.into();
    let derived = derive_uuid(&sample_uuid(111), changed);

    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(preview.add, applied.report.add, "Vorschau und Übernehmen zählen gleich");
    assert_eq!(applied.report.add.revisions, 1, "genau eine weitere Fassung");
    let key = key_of(&store, &sample_uuid(11)).await;
    let row: (i64, String, Option<i64>, String) =
        sqlx::query_as("SELECT number, status, based_on, note FROM revision WHERE uuid = ?")
            .bind(&derived)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(row.0, 5, "neue lokale Nummer last_rev + 1");
    assert_eq!(row.1, "superseded", "„aktuell“ aus der Datei wird abgelöst, nie aktuell (F6 c)");
    assert_eq!(row.2, Some(1), "Basis ist die lokale Original-Fassung");
    assert!(row.3.ends_with("(Austausch: abweichende Fassung aus Probeinstanz)"), "Notiz mit Zusatz: {}", row.3);
    let current: i64 = sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND status = 'current'")
        .bind(key)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(current, 1, "die aktuelle Markierung bleibt bei Fassung 1");
    let deviation = applied.report.deviations.iter().find(|d| d.uuid == derived).expect("Abweichung gelistet");
    assert_eq!(
        (deviation.key, deviation.rev, deviation.action),
        (Some(key), Some(5), DeviationAction::Fassung),
        "Link-Ziel key/rev"
    );
    let original: String = sqlx::query_scalar("SELECT body FROM revision WHERE uuid = ?")
        .bind(sample_uuid(111))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(original, baruch_exchange::sample::SAMPLE_TRICKY_BODY, "die Original-Fassung bleibt unverändert");

    let before = counts(&store).await;
    let third = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(third.report.add.total(), 0, "ein dritter Import legt nichts mehr an");
    assert_eq!(counts(&store).await, before, "keine weitere Zeile");
    let _ = work;
}

#[tokio::test]
async fn outline_changes_become_requests_and_new_points_keep_exactly_their_revisions() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    let outline_before = store.load_work(work).await.unwrap();

    // Umbenennen nur im Titel (keine Fassung trägt ihn), Nummer an/aus, Reihenfolge getauscht.
    node(&mut doc, 12).title = "Das starke Seil".into();
    node(&mut doc, 11).numbered = false;
    let water = node(&mut doc, 10);
    water.children.swap(0, 1);
    // Neuer Abschnitt „Schlinge“ vor „Knoten“, „Knoten“ darunter (Ebene 3).
    let rope = node(&mut doc, 12);
    let mut knot = rope.children.remove(0);
    knot.children.clear();
    let mut noose = ExchangeNode {
        uuid: sample_uuid(500),
        kind: ExchangeNodeKind::Section,
        numbered: true,
        title: "Schlinge".into(),
        revisions: vec![
            ExchangeRevision {
                uuid: sample_uuid(501),
                number: 1,
                status: ExchangeRevisionStatus::Superseded,
                author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
                body: "=== Schlinge\n\nErster Entwurf.\n".into(),
                created: "2026-02-01T09:00:00.000Z".into(),
                ..ExchangeRevision::default()
            },
            ExchangeRevision {
                uuid: sample_uuid(502),
                number: 2,
                status: ExchangeRevisionStatus::Current,
                author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
                based_on: Some(sample_uuid(501)),
                body: "=== Schlinge\n\nZweiter Entwurf.\n".into(),
                created: "2026-02-02T09:00:00.000Z".into(),
                ..ExchangeRevision::default()
            },
        ],
        ..ExchangeNode::default()
    };
    noose.children.push(knot);
    rope.children.push(noose);
    // „Die Lampe“ fehlt in der Datei (samt dem Bezug des Beitrags darauf).
    node(&mut doc, 20).children.clear();
    node(&mut doc, 11).posts[1].refs.clear();

    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(preview.add, applied.report.add, "Vorschau und Übernehmen zählen gleich");
    assert_eq!(preview.requests.len(), applied.report.requests.len(), "gleich viele Anträge");

    // Neuer Punkt mit genau seinen Fassungen (keine zusätzliche Fassung 1 vom Anlegen).
    let noose_key = key_of(&store, &sample_uuid(500)).await;
    let revisions: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT number, status, uuid FROM revision WHERE node_id = ? ORDER BY number")
            .bind(noose_key)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        revisions,
        vec![(1, "superseded".into(), sample_uuid(501)), (2, "current".into(), sample_uuid(502))],
        "genau die zwei Fassungen aus der Datei"
    );
    let parent: i64 = sqlx::query_scalar(
        "SELECT p.id FROM node n JOIN node p ON p.work_id = n.work_id AND p.seq = n.parent_seq WHERE n.id = ?",
    )
    .bind(noose_key)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(parent, key_of(&store, &sample_uuid(12)).await, "an seiner Stelle unter „Das Seil“");

    // Anträge mit Herkunft „Austausch: Probeinstanz“, nichts direkt geändert.
    let requests = store.outline_requests(work, true).await.unwrap();
    let ops: Vec<&OutlineOp> = requests.iter().map(|r| &r.op).collect();
    let seil = key_of(&store, &sample_uuid(12)).await;
    let eimer = key_of(&store, &sample_uuid(11)).await;
    let knoten = key_of(&store, &sample_uuid(13)).await;
    assert!(
        ops.iter()
            .any(|op| matches!(op, OutlineOp::Rename { key, title } if *key == seil && title == "Das starke Seil")),
        "Rename: {ops:?}"
    );
    assert!(
        ops.iter().any(|op| matches!(op, OutlineOp::SetNumbered { key, numbered: false } if *key == eimer)),
        "SetNumbered: {ops:?}"
    );
    assert!(ops.iter().any(|op| matches!(op, OutlineOp::Move { key, .. } if *key == eimer)), "Move: {ops:?}");
    assert!(
        ops.iter().any(|op| matches!(op, OutlineOp::SetLevel { key, level: 3 } if *key == knoten)),
        "SetLevel: {ops:?}"
    );
    for request in &requests {
        assert_eq!(request.origin, IdeaOrigin::Human { name: "Austausch: Probeinstanz".into() }, "Herkunft");
        assert!(
            request.note.starts_with(&format!("Austausch aus Instanz {}", sample_uuid(2))),
            "Notiz: {}",
            request.note
        );
    }
    let outline_after = store.load_work(work).await.unwrap();
    for before in &outline_before.nodes {
        let after = outline_after.nodes.iter().find(|n| n.key == before.key).expect("Punkt bleibt");
        assert_eq!(
            (&after.title, after.parent.is_some()),
            (&before.title, before.parent.is_some()),
            "Titel unverändert"
        );
    }
    let lamp = outline_before.nodes.iter().find(|n| n.title == "Die Lampe").unwrap();
    assert!(
        applied
            .report
            .deviations
            .iter()
            .any(|d| d.key == Some(lamp.key) && d.action == DeviationAction::Hinweis && d.label.contains("fehlt")),
        "fehlender Punkt nur als Hinweis: {:?}",
        applied.report.deviations
    );
    assert!(!ops.iter().any(|op| matches!(op, OutlineOp::Delete { .. })), "kein Antrag zum Löschen");

    // Zweiter Durchgang: dieselben Anträge sind offen → keine neuen, nichts doppelt.
    let before = counts(&store).await;
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.total(), 0, "nichts kommt doppelt");
    assert!(again.requests.is_empty(), "offene Anträge werden nicht doppelt gestellt");
    assert_eq!(counts(&store).await, before, "Zeilenzahlen gleich");
}

#[tokio::test]
async fn a_renamed_point_whose_revision_carries_the_title_only_gets_a_hint() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    import_new(&store, &doc).await;
    // Auf der anderen Seite umbenannt: neuer Titel und eine neue aktuelle Fassung mit dieser Überschrift.
    let lamp = node(&mut doc, 21);
    lamp.title = "Die helle Lampe".into();
    lamp.revisions[0].status = ExchangeRevisionStatus::Superseded;
    lamp.revisions.push(ExchangeRevision {
        uuid: sample_uuid(600),
        number: 2,
        status: ExchangeRevisionStatus::Current,
        author: ExchangeAuthor::Human { name: "Autor".into() },
        based_on: Some(sample_uuid(211)),
        body: "== Die helle Lampe\n\nErfundener Text zu „Die Lampe“.\n".into(),
        created: "2026-02-03T09:00:00.000Z".into(),
        ..ExchangeRevision::default()
    });
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.report.add.revisions, 1, "die neue Fassung kommt (als abgelöst)");
    assert!(applied.requests.is_empty(), "kein Rename-Antrag: die Fassung trägt den Titel, Umschalten genügt");
    assert!(
        applied.report.deviations.iter().any(|d| d.label.contains("Umschalten genügt") && d.rev == Some(2)),
        "Hinweis mit Link auf die Fassung: {:?}",
        applied.report.deviations
    );
}

#[tokio::test]
async fn a_new_part_that_would_adopt_chapters_becomes_an_add_request() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    // Werk ohne Teile: zwei Kapitel oben.
    let bucket = node(&mut doc, 11).clone();
    let rope = node(&mut doc, 12).clone();
    doc.work.outline = vec![bucket, rope];
    doc.work.outline[0].posts[1].refs.clear();
    let work = import_new(&store, &doc).await;
    let nodes_before: i64 = sqlx::query_scalar("SELECT count(*) FROM node WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    // Die andere Seite hat einen Teil um beide Kapitel gelegt.
    let chapters = std::mem::take(&mut doc.work.outline);
    doc.work.outline = vec![ExchangeNode {
        uuid: sample_uuid(700),
        kind: ExchangeNodeKind::Part,
        numbered: true,
        title: "Ganz neuer Teil".into(),
        revisions: vec![ExchangeRevision {
            uuid: sample_uuid(701),
            number: 1,
            status: ExchangeRevisionStatus::Current,
            author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
            body: "#heading(level: 1, numbering: none)[Ganz neuer Teil] <baruch-part>\n".into(),
            created: "2026-02-04T09:00:00.000Z".into(),
            ..ExchangeRevision::default()
        }],
        children: chapters,
        ..ExchangeNode::default()
    }];
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    let nodes_after: i64 = sqlx::query_scalar("SELECT count(*) FROM node WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(nodes_after, nodes_before, "der Teil wird nicht angelegt, er braucht die Zustimmung des Menschen");
    let requests = store.outline_requests(work, true).await.unwrap();
    assert_eq!(requests.len(), 1, "genau ein Antrag: {requests:?}");
    assert!(
        matches!(&requests[0].op, OutlineOp::Add { points, .. } if points[0].title == "Ganz neuer Teil"),
        "Antrag Add"
    );
    assert!(requests[0].op.needs_consent(), "Zustimmung im Dialog nötig (AGENTS.md)");
    assert_eq!(requests[0].expected.adopted.len(), 2, "beide Kapitel würden wandern");
    let deviation = applied.report.deviations.iter().find(|d| d.uuid == sample_uuid(700)).expect("gemeldet");
    assert_eq!(deviation.action, DeviationAction::Antrag, "als Antrag gemeldet");
    // Runde 22 (§2.10): Kennung und Fassungen reisen im Antrag mit (Zustimmen und erneuter Abgleich in
    // tests/exchange_u8.rs); das sagt die Vorschau.
    assert!(
        deviation.there.contains("Antrag mit Kennung und 1 Fassung(en)"),
        "Kennung und Fassungen reisen im Antrag mit, und das wird gesagt: {}",
        deviation.there
    );
    assert!(
        matches!(&requests[0].op, OutlineOp::Add { part: Some(part), .. } if part.uuid == sample_uuid(700)),
        "der Antrag trägt die Kennung des Teils"
    );
}

#[tokio::test]
async fn tombstones_block_recreation_and_new_things_arrive_while_differences_only_hint() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier gelöscht: die verworfene Fassung 4 von „Der Eimer“ und der zweite Beitrag (Grabsteine per Trigger).
    let seq = store.node_seq(work, key_of(&store, &sample_uuid(11)).await).await.unwrap();
    store.delete_revision(work, seq, 4, "Autor").await.unwrap();
    let post_id: i64 = sqlx::query_scalar("SELECT id FROM post WHERE uuid = ?")
        .bind(sample_uuid(31))
        .fetch_one(store.pool())
        .await
        .unwrap();
    store.post_delete(work, post_id).await.unwrap();
    // Buchangaben und Sprechdauer fehlen hier.
    sqlx::query("DELETE FROM work_book WHERE work_id = ?").bind(work).execute(store.pool()).await.unwrap();
    sqlx::query("DELETE FROM work_timing WHERE work_id = ?").bind(work).execute(store.pool()).await.unwrap();

    // Die andere Seite: neuer Einfall, neuer Beitrag, neuer Fadenbeitrag, neue Bildfassung; Abweichungen.
    doc.work.ideas.push(ExchangeIdea {
        uuid: sample_uuid(800),
        words: vec!["Eimer".into()],
        note: "Neuer Gedanke von drüben.".into(),
        origin_kind: ExchangeOriginKind::Human,
        origin_name: "Hanna Probe".into(),
        status: ExchangeIdeaStatus::Open,
        created: "2026-02-05T09:00:00.000Z".into(),
        updated: "2026-02-05T09:00:00.000Z".into(),
        ..ExchangeIdea::default()
    });
    doc.work.ideas[0].note = "Das Seil reißt manchmal doch.".into();
    node(&mut doc, 21).posts.push(ExchangePost {
        uuid: sample_uuid(801),
        position: 1,
        author: "@lektor".into(),
        body: "Die Lampe flackert.".into(),
        created: "2026-02-05T10:00:00.000Z".into(),
        ..ExchangePost::default()
    });
    doc.work.threads[0].posts.push(baruch_exchange::ExchangeThreadPost {
        uuid: sample_uuid(802),
        position: 3,
        author: "@lektor".into(),
        body: "Und wie breit?".into(),
        created: "2026-02-05T11:00:00.000Z".into(),
        ..Default::default()
    });
    let png = &mut doc.work.assets[0];
    png.revisions[1].caption = "Der Brunnen am Morgen".into();
    png.revisions.push(ExchangeAssetRevision {
        uuid: sample_uuid(803),
        number: 3,
        based_on: Some(sample_uuid(62)),
        mime: "image/png".into(),
        sha256: sha256_hex(SAMPLE_PNG_GREEN),
        data: SAMPLE_PNG_GREEN.to_vec(),
        author: "Hanna Probe".into(),
        status: ExchangeAssetStatus::Superseded,
        created: "2026-02-05T12:00:00.000Z".into(),
        ..ExchangeAssetRevision::default()
    });
    // Ein Bild, dessen Bytes kein Bild sind, besteht die Prüfung wie beim Hochladen nicht.
    doc.work.assets.push(baruch_exchange::ExchangeAsset {
        uuid: sample_uuid(804),
        name: "falsch.png".into(),
        created: "2026-02-05T12:00:00.000Z".into(),
        paths: Vec::new(),
        revisions: vec![ExchangeAssetRevision {
            uuid: sample_uuid(805),
            number: 1,
            mime: "image/png".into(),
            sha256: sha256_hex(b"kein Bild"),
            data: b"kein Bild".to_vec(),
            author: "Hanna Probe".into(),
            status: ExchangeAssetStatus::Current,
            created: "2026-02-05T12:00:00.000Z".into(),
            ..ExchangeAssetRevision::default()
        }],
    });
    doc.work.subtitle = "Ein anders erfundener Untertitel".into();

    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    let report = &applied.report;
    let deleted: Vec<&str> = report.deleted_here.iter().map(|d| d.uuid.as_str()).collect();
    assert!(deleted.contains(&sample_uuid(114).as_str()), "gelöschte Fassung steht in deleted_here: {deleted:?}");
    assert!(deleted.contains(&sample_uuid(31).as_str()), "gelöschter Beitrag steht in deleted_here: {deleted:?}");
    let back: i64 = sqlx::query_scalar("SELECT count(*) FROM revision WHERE uuid = ?")
        .bind(sample_uuid(114))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(back, 0, "die gelöschte Fassung kommt nicht wieder");
    assert_eq!(
        (report.add.ideas, report.add.posts, report.add.thread_posts, report.add.asset_revisions, report.add.assets),
        (1, 1, 1, 1, 0),
        "neuer Einfall, Beitrag, Fadenbeitrag und Bildfassung; das kaputte Bild nicht"
    );
    assert!(report.refusals.iter().any(|r| r.contains("falsch.png")), "Bildprüfung abgelehnt: {:?}", report.refusals);
    let new_png: String = sqlx::query_scalar("SELECT status FROM asset_revision WHERE uuid = ?")
        .bind(sample_uuid(803))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(new_png, "superseded", "eingelesene Bildfassung wird nicht aktuell");
    let caption: String = sqlx::query_scalar("SELECT caption FROM asset_revision WHERE uuid = ?")
        .bind(sample_uuid(62))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(caption, "Der Brunnen im Abendlicht", "abweichende Bildunterschrift bleibt");
    let note: String = sqlx::query_scalar("SELECT note FROM idea WHERE uuid = ?")
        .bind(sample_uuid(40))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(note, "Das Seil reißt nie ganz.", "abweichender Einfall bleibt");
    for word in ["Bildunterschrift", "Notiz weicht ab", "Untertitel"] {
        assert!(
            report.deviations.iter().any(|d| d.label.contains(word) && d.action == DeviationAction::Hinweis),
            "Hinweis zu {word}: {:?}",
            report.deviations
        );
    }
    let book: i64 = sqlx::query_scalar("SELECT count(*) FROM work_book WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let timing: i64 = sqlx::query_scalar("SELECT count(*) FROM work_timing WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!((book, timing), (1, 1), "fehlende Buch- und Sprechdauerzeile werden angelegt");
}

#[tokio::test]
async fn merging_never_overwrites_an_existing_row() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    import_new(&store, &doc).await;
    // Viele Abweichungen auf einmal: Text, Titel, Nummer, neuer Abschnitt, neue Einfälle, Unterschrift.
    node(&mut doc, 11).revisions[1].body = "== Der Eimer\n\nDer Eimer war halb voll.\n".into();
    node(&mut doc, 11).revisions[0].note = "Andere Notiz.".into();
    node(&mut doc, 12).title = "Das lange Seil".into();
    node(&mut doc, 13).numbered = false;
    node(&mut doc, 21).children.push(ExchangeNode {
        uuid: sample_uuid(900),
        kind: ExchangeNodeKind::Section,
        numbered: true,
        title: "Docht".into(),
        revisions: vec![ExchangeRevision {
            uuid: sample_uuid(901),
            number: 1,
            status: ExchangeRevisionStatus::Current,
            author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
            body: "=== Docht\n\nKurz.\n".into(),
            created: "2026-02-06T09:00:00.000Z".into(),
            ..ExchangeRevision::default()
        }],
        ..ExchangeNode::default()
    });
    doc.work.ideas[1].quote = "Eine andere Lampe".into();
    doc.work.assets[1].revisions[0].alt = "Anders beschrieben".into();
    doc.work.template.current = "#set text(lang: \"en\")\n".into();
    // Vor dem Abgleich alles abziehen. Ausgenommen sind nur die Spalten, die das zulässige Anlegen eines Punkts
    // bzw. einer Fassung neu rechnet: seq, parent_seq, position und number der Punkte (Stelle in der Gliederung,
    // hier nach dem neuen Abschnitt verschoben), last_rev (höchste vergebene Nummer) und node_seq der Bezüge
    // (folgt der Stelle). Alle anderen Spalten aller vorhandenen Zeilen müssen gleich bleiben.
    let skip = [
        ("node", "seq"),
        ("node", "parent_seq"),
        ("node", "position"),
        ("node", "number"),
        ("node", "last_rev"),
        ("asset", "last_rev"),
        ("post_ref", "node_seq"),
    ];
    let before = dump(&store, &skip).await;
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert!(applied.report.add.total() >= 2, "etwas kam hinzu (Abschnitt, Fassung): {:?}", applied.report.add);
    let after = dump(&store, &skip).await;
    for (row, value) in &before {
        assert_eq!(after.get(row), Some(value), "Zeile {row} wurde verändert oder gelöscht");
    }
    assert!(after.len() > before.len(), "nur neue Zeilen kamen hinzu");
}

#[tokio::test]
async fn an_invalid_document_writes_nothing() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    // Zwei aktuelle Fassungen in einem Punkt verletzen die Regel „genau eine aktuelle Fassung“.
    node(&mut doc, 11).revisions[1].status = ExchangeRevisionStatus::Current;
    let before = counts(&store).await;
    let refused = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await;
    assert!(matches!(refused, Err(StoreError::Exchange(_))), "ungültig → Exchange: {refused:?}");
    assert_eq!(counts(&store).await, before, "nichts geschrieben");
}

#[tokio::test]
async fn a_done_ref_naming_a_point_becomes_its_local_key_again() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    // Die Ausgabe schreibt „key N, Rest“ als „<UUID des Punkts>, Rest“; das Einlesen macht daraus den Schlüssel hier.
    doc.work.ideas[0].status = ExchangeIdeaStatus::Done;
    doc.work.ideas[0].done_as = Some(baruch_exchange::ExchangeIdeaUse::Outline);
    doc.work.ideas[0].done_ref = Some(format!("{}, als Abschnitt", sample_uuid(13)));
    import_new(&store, &doc).await;
    let key = key_of(&store, &sample_uuid(13)).await;
    let done_ref: Option<String> = sqlx::query_scalar("SELECT done_ref FROM idea WHERE uuid = ?")
        .bind(sample_uuid(40))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        done_ref,
        Some(format!("key {key}, als Abschnitt")),
        "Kennung → key mit dem Schlüssel hier, Rest wörtlich"
    );
}

// ---- Unterversion 3 (Runde 27): Umschlag-Anordnung, Anschriften, Satzbild ----------------------------

#[tokio::test]
async fn a_new_import_carries_cover_blocks_addresses_and_sheet_and_dumps_them_back() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let work = applied.work;
    assert_eq!(applied.report.add.cover_blocks, 3, "drei Umschlag-Blöcke gezählt");
    // Blöcke mit ihren Kennungen und Werten (Farbe in der gespeicherten Form, Laufrichtung, eigener Text).
    type BlockRow = (String, String, f64, String, Option<String>, Option<String>, String);
    let rows: Vec<BlockRow> = sqlx::query_as(
        "SELECT uuid, kind, x_mm, color, direction, text, updated FROM cover_block WHERE work_id = ? ORDER BY id",
    )
    .bind(work)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let kinds: Vec<(&str, &str)> = rows.iter().map(|r| (r.0.as_str(), r.1.as_str())).collect();
    let expected =
        [(sample_uuid(160), "front_title"), (sample_uuid(161), "spine_title"), (sample_uuid(162), "back_motto")];
    assert_eq!(
        kinds,
        expected.iter().map(|(u, k)| (u.as_str(), *k)).collect::<Vec<_>>(),
        "jeder Block behält seine Kennung aus der Datei"
    );
    assert_eq!((rows[1].3.as_str(), rows[1].4.as_deref()), ("dark", Some("up")), "Farbe und Laufrichtung des Rückens");
    assert_eq!((rows[2].2, rows[2].3.as_str()), (-2.5, "#1f2a33"), "negative Lage (Beschnitt) und Hex-Farbe");
    assert_eq!(rows[2].5.as_deref(), Some("Erfundenes Motto & <Spruch>\nzweite Zeile"), "eigener Text exakt");
    assert_eq!(rows[0].6, "2026-01-08T16:00:00.000Z", "Zeitpunkt aus der Datei");
    let meta = store.book_meta(work).await.unwrap();
    assert_eq!(meta.publisher_address, "Probeverlag\nAm Brunnen 1\n12345 Musterstadt", "Anschrift des Verlags");
    assert_eq!(meta.printer_address, "Probedruck\nDruckgasse 2\n12345 Musterstadt", "Anschrift der Druckerei");
    let sheet = store.sheet_settings(work).await.unwrap();
    assert_eq!(
        (sheet.font.as_deref(), sheet.size.as_deref(), sheet.background.as_deref()),
        (Some("\"Erfundene Antiqua\", serif"), Some("11pt"), Some("#fbf8f1")),
        "Satzbild-Einstellungen (Punkt aus „Nach Runde 18 offen“)"
    );
    let stone: (String, Option<String>) = sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid = ?")
        .bind(sample_uuid(163))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(stone, ("cover_block".into(), Some(sample_uuid(1))), "Grabstein umschlag-block mitgenommen");
    // Rundreise im Store: die Ausgabe trägt genau, was die Datei brachte.
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.book, doc.work.book, "Buchangaben samt Anschriften und Blöcken zurück");
    assert_eq!(dumped.sheet, doc.work.sheet, "Satzbild zurück");
    assert!(dumped.deleted.iter().any(|t| t.uuid == sample_uuid(163)), "Grabstein zurück");
}

#[tokio::test]
async fn merging_cover_blocks_adds_missing_kinds_keeps_local_ones_and_respects_tombstones() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier: Rücken zurückgesetzt (Grabstein für 161), Untertitel lokal angelegt, Satzbild lokal anders.
    store.reset_cover_blocks(work, &[baruch_store::CoverFace::Spine], None).await.unwrap();
    let local_subtitle = baruch_store::CoverBlock {
        kind: baruch_store::CoverBlockKind::FrontSubtitle,
        x_mm: 10.0,
        y_mm: 60.0,
        width_mm: 100.0,
        align: baruch_typst::cover_layout::CoverAlign::Left,
        size_pt: 14.0,
        leading_em: 0.65,
        color: "light".into(),
        font: None,
        style: baruch_typst::cover_layout::FontStyle::Normal,
        backdrop: baruch_typst::cover_layout::Backdrop::None,
        visible: true,
        direction: None,
        text: None,
    };
    store.set_cover_blocks(work, std::slice::from_ref(&local_subtitle)).await.unwrap();
    let sheet = baruch_store::SheetSettings { size: Some("12pt".into()), ..Default::default() };
    store.set_sheet_settings(work, &sheet).await.unwrap();
    // Dort: Titel verschoben (bekannte Kennung), neuer Autor-Block, eigener Untertitel-Block, Anschrift anders.
    let book = doc.work.book.as_mut().unwrap();
    book.cover_blocks[0].x_mm = 40.0;
    book.publisher_address = "Anderer Verlag\nAnderswo 3".into();
    let mut author = book.cover_blocks[0].clone();
    author.uuid = sample_uuid(171);
    author.kind = baruch_exchange::ExchangeCoverBlockKind::FrontAuthor;
    author.font = None;
    let mut subtitle = book.cover_blocks[0].clone();
    subtitle.uuid = sample_uuid(172);
    subtitle.kind = baruch_exchange::ExchangeCoverBlockKind::FrontSubtitle;
    book.cover_blocks.push(author);
    book.cover_blocks.push(subtitle);
    let before = store.cover_layout(work).await.unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    let report = &applied.report;
    assert_eq!(report.add.cover_blocks, 1, "nur der Autor-Block kommt dazu: {:?}", report.items);
    assert!(report.items.iter().any(|i| i.uuid == sample_uuid(171) && i.kind == "umschlag-block"), "Autor gelistet");
    let after = store.cover_layout(work).await.unwrap();
    assert_eq!(
        after.get(baruch_store::CoverBlockKind::FrontTitle),
        before.get(baruch_store::CoverBlockKind::FrontTitle),
        "bekannter Titel-Block wird nie überschrieben"
    );
    assert_eq!(after.get(baruch_store::CoverBlockKind::FrontSubtitle), Some(&local_subtitle), "lokal gewinnt");
    assert!(after.get(baruch_store::CoverBlockKind::SpineTitle).is_none(), "zurückgesetzter Rücken kommt nicht wieder");
    assert!(
        report.deleted_here.iter().any(|d| d.uuid == sample_uuid(161) && d.kind == "umschlag-block"),
        "Grabstein → „hier gelöscht“: {:?}",
        report.deleted_here
    );
    let hint = |uuid: String, needle: &str| {
        report
            .deviations
            .iter()
            .any(|d| d.uuid == uuid && d.label.contains(needle) && d.action == DeviationAction::Hinweis)
    };
    assert!(hint(sample_uuid(160), "weicht ab"), "verschobener Titel: nur Hinweis: {:?}", report.deviations);
    assert!(hint(sample_uuid(172), "lokal gewinnt"), "zweiter Untertitel: Hinweis: {:?}", report.deviations);
    assert!(hint(sample_uuid(1), "Anschrift des Verlags"), "Anschrift weicht ab: Hinweis: {:?}", report.deviations);
    assert!(hint(sample_uuid(1), "Satzbild"), "Satzbild weicht ab: Hinweis: {:?}", report.deviations);
    assert_eq!(store.sheet_settings(work).await.unwrap(), sheet, "lokales Satzbild bleibt");
    assert_eq!(store.book_meta(work).await.unwrap().publisher_address.lines().count(), 3, "lokale Anschrift bleibt");
    // Ein zweiter Durchgang fügt nichts mehr hinzu.
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.total(), 0, "nichts doppelt: {:?}", again.report.items);
}

#[tokio::test]
async fn merging_brings_a_missing_sheet_and_missing_book_with_blocks() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier gibt es (noch) kein Satzbild und keine Buchangaben samt Blöcken: beides fehlt ganz und kommt aus der Datei.
    store.set_sheet_settings(work, &baruch_store::SheetSettings::default()).await.unwrap();
    store.reset_cover_blocks(work, &[], None).await.unwrap();
    sqlx::query("DELETE FROM work_book WHERE work_id = ?").bind(work).execute(store.pool()).await.unwrap();
    sqlx::query("DELETE FROM uuid_tombstone WHERE kind = 'cover_block' AND uuid <> ?")
        .bind(sample_uuid(163))
        .execute(store.pool())
        .await
        .unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.report.add.cover_blocks, 3, "ohne Grabstein kommen alle drei Blöcke");
    assert!(store.sheet_settings(work).await.unwrap().size.is_some(), "Satzbild aus der Datei angelegt");
    assert_eq!(store.book_meta(work).await.unwrap().printer_address.lines().count(), 3, "Anschriften mit dem Buch");
}

#[tokio::test]
async fn an_invalid_sheet_in_the_file_is_not_taken_but_reported() {
    // Die Datei kommt von außen: Ein Satzbild, das lokal nicht durchginge, wird nicht übernommen (wie ein Bild, das
    // die Prüfung nicht besteht), das Werk aber schon.
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    doc.work.sheet = Some(baruch_exchange::ExchangeSheet { font: Some("A;} body {".into()), ..Default::default() });
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    assert!(store.sheet_settings(applied.work).await.unwrap().is_empty(), "nichts übernommen");
    assert!(
        applied.report.refusals.iter().any(|r| r.contains("Satzbild")),
        "unter „Nicht übernommen“: {:?}",
        applied.report.refusals
    );
}

// ---- Unterversion 4 (Runde 28): Befunde und alte Beitragsfassungen ----------------------------------------

#[tokio::test]
async fn a_new_import_carries_findings_history_and_post_versions_and_dumps_them_back() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let work = applied.work;
    let add = &applied.report.add;
    assert_eq!((add.findings, add.finding_entries, add.post_versions), (5, 4, 2), "Befunde, Verlauf, alte Fassungen");
    // Befunde mit Kennung, Status, Begründung; „gleicht“ über die Kennung (das Original steht davor in der Datei).
    let rows: Vec<(String, String, String, Option<i64>, String)> = sqlx::query_as(
        "SELECT f.uuid, f.status, f.reason, f.similar_to, f.summary_of FROM finding f WHERE f.work_id = ? ORDER BY f.id",
    )
    .bind(work)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 5, "alle fünf Befunde");
    assert_eq!(
        (rows[0].1.as_str(), rows[0].2.as_str()),
        ("rejected", "Absicht: der Eimer bleibt glatt.\nZweite Zeile.")
    );
    let original: i64 = sqlx::query_scalar("SELECT id FROM finding WHERE uuid = ?")
        .bind(sample_uuid(311))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(rows[3].3, Some(original), "angehängter Befund zeigt auf das Original hier");
    assert!(rows[3].4.contains(&sample_uuid(310)), "Sammlung über Kennungen: {}", rows[3].4);
    // Der übernommene Befund findet seine Fassung (Nummer 1 von „Der Eimer“) hier wieder.
    let key = key_of(&store, &sample_uuid(11)).await;
    assert_eq!(
        store.findings_adopted_in(work, key).await.unwrap(),
        vec![(1, vec![original])],
        "Fassung 1 setzt ihn um"
    );
    // Beitrag mit alter Fassung und Zeit der Bearbeitung.
    let edited: Option<String> = sqlx::query_scalar("SELECT edited FROM post WHERE uuid = ?")
        .bind(sample_uuid(31))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(edited.as_deref(), Some("2026-01-07T13:00:00.000Z"), "post.edited aus der Datei");
    // Rundreise im Store: die Ausgabe trägt genau, was die Datei brachte.
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.findings, doc.work.findings, "Befunde samt Verlauf zurück");
    // Runde 30: Der Schlüssel eines Punkts gilt nur in seiner Instanz (`schluessel` der Datei bleibt dort); alles
    // andere – Varianten, Lösungen, Grundlage und Antwort der Beiträge – kommt genau zurück.
    fn without_keys(nodes: &[baruch_exchange::ExchangeNode]) -> Vec<baruch_exchange::ExchangeNode> {
        nodes
            .iter()
            .map(|n| baruch_exchange::ExchangeNode { key: None, children: without_keys(&n.children), ..n.clone() })
            .collect()
    }
    assert_eq!(without_keys(&dumped.outline), without_keys(&doc.work.outline), "Beiträge samt alten Fassungen zurück");
    assert!(dumped.outline.iter().all(|n| n.key.is_some()), "die Ausgabe nennt die Schlüssel dieser Instanz");
    assert_eq!(dumped.threads, doc.work.threads, "Fadenbeiträge samt alten Fassungen zurück");
    for n in [315, 324, 302] {
        assert!(dumped.deleted.iter().any(|t| t.uuid == sample_uuid(n)), "Grabstein {n} der Unterversion 4 zurück");
    }
}

#[tokio::test]
async fn merging_findings_adds_unknown_ones_never_overwrites_and_respects_tombstones_and_taken_positions() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier: den Hinweis aus dem Faden löschen (Grabstein), den verschobenen Befund wieder öffnen (abweichender Status).
    sqlx::query("DELETE FROM finding WHERE uuid = ?").bind(sample_uuid(313)).execute(store.pool()).await.unwrap();
    let deferred: i64 = sqlx::query_scalar("SELECT id FROM finding WHERE uuid = ?")
        .bind(sample_uuid(312))
        .fetch_one(store.pool())
        .await
        .unwrap();
    store.finding_set_status(work, deferred, &baruch_store::StatusChange::Open, "Autor").await.unwrap();
    let before = counts(&store).await;
    // Drüben: ein neuer Befund an freier Stelle (mit Verlauf), einer an einer hier schon belegten Stelle (andere
    // Kennung – etwa weil beide Instanzen denselben Beitrag zerlegt haben), eine neue alte Fassung eines bekannten
    // Beitrags.
    let mut fresh = doc.work.findings[1].clone();
    fresh.uuid = sample_uuid(700);
    fresh.position = 7;
    fresh.history[0].uuid = sample_uuid(701);
    let mut twin = doc.work.findings[1].clone();
    twin.uuid = sample_uuid(702);
    twin.history.clear();
    doc.work.findings.push(fresh);
    doc.work.findings.push(twin);
    node(&mut doc, 11).posts[1].versions.push(baruch_exchange::ExchangePostVersion {
        uuid: sample_uuid(703),
        body: "Noch ältere Fassung.".into(),
        valid_from: "2026-01-07T12:30:00.000Z".into(),
        replaced: "2026-01-07T12:45:00.000Z".into(),
        edited_by: "Hanna Probe".into(),
    });
    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(preview.add, applied.report.add, "Vorschau und Übernehmen zählen gleich");
    let add = &applied.report.add;
    assert_eq!(
        (add.findings, add.finding_entries, add.post_versions),
        (1, 1, 1),
        "nur der neue Befund mit seinem Eintrag und die neue alte Fassung"
    );
    let after = counts(&store).await;
    assert_eq!(after["finding"], before["finding"] + 1, "genau ein Befund mehr");
    assert_eq!(after["post_version"], before["post_version"] + 1, "genau eine alte Fassung mehr");
    let report = &applied.report;
    assert!(
        report.deleted_here.iter().any(|d| d.uuid == sample_uuid(313) && d.kind == "befund"),
        "der hier gelöschte Befund kommt nicht wieder: {:?}",
        report.deleted_here
    );
    assert!(
        report.deviations.iter().any(|d| d.uuid == sample_uuid(312) && d.label.contains("Status weicht ab")),
        "abweichender Status nur als Hinweis: {:?}",
        report.deviations
    );
    assert!(
        report.deviations.iter().any(|d| d.uuid == sample_uuid(702) && d.label.contains("schon Befund")),
        "belegte Stelle nicht doppelt: {:?}",
        report.deviations
    );
    let status: String = sqlx::query_scalar("SELECT status FROM finding WHERE id = ?")
        .bind(deferred)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "open", "der bekannte Befund wird nie überschrieben (den Status setzt hier der Mensch)");
    // Ein zweites Abgleichen bringt nichts mehr.
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(again.report.add.total(), 0, "nichts doppelt: {:?}", again.report.items);
}

// ---- Unterversion 7 (Runde 31): eigene Register und Glossar ---------------------------------------------------

#[tokio::test]
async fn a_new_import_carries_registers_and_glossary_and_dumps_them_back() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let work = applied.work;
    assert_eq!(
        (applied.report.add.registers, applied.report.add.glossary_terms),
        (2, 3),
        "gezählt: {:?}",
        applied.report.add
    );
    let registers = store.index_registers(work).await.unwrap();
    assert_eq!(
        registers.iter().map(|r| (r.uuid.as_str(), r.slug.as_str())).collect::<Vec<_>>(),
        [(sample_uuid(1110).as_str(), "orte"), (sample_uuid(1111).as_str(), "werkzeuge")],
        "Register mit ihren Kennungen in ihrer Reihenfolge"
    );
    let terms = store.glossary(work, None).await.unwrap();
    let proposal = terms.iter().find(|t| t.term == "Zisterne").expect("Vorschlag übernommen");
    assert_eq!(
        (proposal.status, proposal.origin.as_str(), proposal.uuid.as_str()),
        (baruch_store::GlossaryStatus::Proposed, "@lektor", sample_uuid(1102).as_str()),
        "Status, Herkunft und Kennung wie in der Datei"
    );
    assert_eq!(store.glossary_lookup(work, "Brunnenstuben").await.unwrap().len(), 1, "Variante nachschlagbar");
    for (uuid, kind) in [(sample_uuid(1103), "glossary_term"), (sample_uuid(1112), "index_register")] {
        let stone: String = sqlx::query_scalar("SELECT kind FROM uuid_tombstone WHERE uuid = ?")
            .bind(&uuid)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(stone, kind, "Grabstein {kind} mitgenommen");
    }
    // Rundreise im Store: die Ausgabe trägt genau, was die Datei brachte.
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.registers, doc.work.registers, "eigene Register zurück");
    assert_eq!(dumped.glossary, doc.work.glossary, "Glossar zurück (Varianten, Status, Zeiten)");
    assert_eq!(dumped.typeset, doc.work.typeset, "Satz-Einstellungen samt R31-Feldern zurück");
}

#[tokio::test]
async fn merging_the_glossary_matches_instead_of_doubling_and_never_overwrites() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier gelöscht: „Schöpfeimer“ (Grabstein) – er darf nicht wiederkommen.
    let schoepfeimer = store.glossary(work, None).await.unwrap().into_iter().find(|t| t.term == "Schöpfeimer").unwrap();
    store.glossary_delete(work, schoepfeimer.id, "Hanna").await.unwrap();
    let before = counts(&store).await;

    // Die Datei der anderen Instanz: derselbe Begriff „Brunnenstube“ unter eigener Kennung (beide Instanzen haben ihn
    // angelegt), der bekannte Vorschlag „Zisterne“ mit anderer Erklärung, der hier gelöschte „Schöpfeimer“, ein neuer
    // Begriff; das Register „orte“ unter eigener Kennung, ein neues Register.
    doc.work.glossary[0].uuid = sample_uuid(1120);
    doc.work.glossary[0].term = "BRUNNENSTUBE".into();
    doc.work.glossary[2].short = "Andere Erklärung dort.".into();
    let mut fresh = doc.work.glossary[1].clone();
    fresh.uuid = sample_uuid(1121);
    fresh.term = "Seilwinde".into();
    doc.work.glossary.push(fresh);
    doc.work.registers[0].uuid = sample_uuid(1122);
    doc.work.registers[1].uuid = sample_uuid(1123);
    doc.work.registers[1].slug = "pflanzen".into();

    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(preview.add, applied.report.add, "Vorschau und Übernehmen zählen gleich");
    let report = &applied.report;
    assert_eq!(
        (report.add.glossary_terms, report.add.registers),
        (1, 1),
        "nur Seilwinde und pflanzen: {:?}",
        report.add
    );
    let after = counts(&store).await;
    assert_eq!(after["glossary_term"], before["glossary_term"] + 1, "ein Begriff mehr, kein Doppel");
    assert_eq!(after["index_register"], before["index_register"] + 1, "ein Register mehr, kein Doppel");
    let hint = |uuid: &str| report.deviations.iter().any(|d| d.uuid == uuid && d.action == DeviationAction::Hinweis);
    assert!(hint(&sample_uuid(1102)), "abweichender bekannter Begriff: nur ein Hinweis: {:?}", report.deviations);
    assert!(hint(&sample_uuid(1120)), "gleicher Begriff unter neuer Kennung: Hinweis statt Doppel");
    assert!(hint(&sample_uuid(1122)), "gleiche Register-Kennung unter neuer uuid: Hinweis statt Doppel");
    assert!(
        report.deleted_here.iter().any(|d| d.uuid == schoepfeimer.uuid && d.kind == "glossar-begriff"),
        "hier gelöschter Begriff kommt nicht wieder: {:?}",
        report.deleted_here
    );
    let zisterne = store.glossary(work, None).await.unwrap().into_iter().find(|t| t.term == "Zisterne").unwrap();
    assert_ne!(zisterne.short, "Andere Erklärung dort.", "bekannter Begriff wird nie überschrieben");
    assert_eq!(store.glossary_lookup(work, "brunnenstube").await.unwrap().len(), 1, "Brunnenstube nur einmal");
    assert!(store.glossary_lookup(work, "Schöpfeimer").await.unwrap().is_empty(), "Grabstein respektiert");
    assert!(store.index_registers(work).await.unwrap().iter().any(|r| r.slug == "pflanzen"), "neues Register da");

    // Ein zweites Mal: nichts kommt mehr dazu.
    let again = store.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!((again.report.add.glossary_terms, again.report.add.registers), (0, 0), "zweiter Abgleich bringt nichts");
}
