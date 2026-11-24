// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gedankenkarte im Austauschformat (Runde 25, Vertrag §4.6): Verbindungen und Lagen mit ihren Kennungen beim
//! Ausgeben (`Store::exchange_dump`), beim Einlesen als neues Werk und beim Abgleichen (`Store::exchange_preview`,
//! `Store::exchange_apply`). Ausgangspunkt ist das erfundene Beispielwerk aus `baruch_exchange::sample` (zwei
//! Verbindungen, zwei Lagen, je ein Grabstein `verbindung` und `lage`). Verbindungen und Lagen, die es „hier“ schon
//! gibt, entstehen über SQL, weil die Schreibwege der Karte (karte-daten) in diesem Zweig noch Stubs sind; die
//! Fülltrigger aus Migration 0043 vergeben ihre Kennungen wie auf jedem anderen Weg. Jede Zusicherung mit Begründung.

use baruch_exchange::sample::{sample_document, sample_uuid};
use baruch_exchange::{ExchangeDocument, ExchangeLink, ExchangeLinkKind, ExchangePosition, ExchangeTombstoneKind};
use baruch_store::exchange_apply::{ApplyMode, ExchangeReport};
use baruch_store::{Store, WorkId};

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000025";

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn import_new(store: &Store, doc: &ExchangeDocument) -> WorkId {
    store.exchange_apply(doc, SHA, ApplyMode::New, None).await.unwrap().work
}

async fn merge(store: &Store, doc: &ExchangeDocument) -> ExchangeReport {
    store.exchange_apply(doc, SHA, ApplyMode::Merge, None).await.unwrap().report
}

async fn count(store: &Store, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(store.pool()).await.unwrap()
}

async fn idea_id(store: &Store, n: u32) -> i64 {
    sqlx::query_scalar("SELECT id FROM idea WHERE uuid = ?").bind(sample_uuid(n)).fetch_one(store.pool()).await.unwrap()
}

/// Verbindung (from, to, kind, note, deleted) zur Kennung.
async fn link_row(store: &Store, uuid: &str) -> Option<(i64, i64, String, String, Option<String>)> {
    sqlx::query_as("SELECT from_id, to_id, kind, note, deleted FROM idea_link WHERE uuid = ?")
        .bind(uuid)
        .fetch_optional(store.pool())
        .await
        .unwrap()
}

fn link(n: u32, from: u32, to: u32, kind: ExchangeLinkKind) -> ExchangeLink {
    ExchangeLink {
        uuid: sample_uuid(n),
        from: sample_uuid(from),
        to: sample_uuid(to),
        kind,
        note: format!("Probe {n}"),
        origin_kind: baruch_exchange::ExchangeOriginKind::Mcp,
        origin_name: "@mcp:probe".into(),
        created: "2026-01-09T10:00:00.000Z".into(),
        updated: "2026-01-09T10:00:00.000Z".into(),
        deleted: None,
    }
}

#[tokio::test]
async fn a_new_import_takes_links_and_positions_with_their_uuids() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    assert_eq!((preview.add.links, preview.add.positions), (2, 2), "die Vorschau zählt zwei Verbindungen und Lagen");
    assert_eq!(count(&store, "idea_link").await, 0, "die Vorschau schreibt nichts");
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    assert_eq!(applied.report.add, preview.add, "Vorschau und Übernehmen zählen gleich");
    assert!(
        applied.report.add.total() >= applied.report.add.links + applied.report.add.positions + 6,
        "total() zählt Verbindungen und Lagen mit"
    );
    assert!(
        applied.report.items.iter().any(|i| i.kind == "verbindung" && i.label.contains("führt weiter")),
        "die Liste nennt die Verbindung mit ihrer Art: {:?}",
        applied.report.items
    );

    let (seil, lampe, eimer) = (idea_id(&store, 40).await, idea_id(&store, 41).await, idea_id(&store, 42).await);
    let leads = link_row(&store, &sample_uuid(150)).await.expect("Verbindung 150 mit ihrer Kennung");
    assert_eq!(
        leads,
        (seil, lampe, "leads_to".into(), "Vom Seil zur Lampe.".into(), None),
        "führt weiter: Richtung, Art, Notiz wie in der Datei"
    );
    let contra = link_row(&store, &sample_uuid(151)).await.expect("Verbindung 151 mit ihrer Kennung");
    assert_eq!((contra.0, contra.1), (lampe.min(eimer), lampe.max(eimer)), "widerspricht normalisiert (from < to)");
    assert_eq!(contra.3, "Eimer & <Lampe>\nstreiten sich.", "Notiz exakt mit Zeilenumbruch");
    assert!(contra.4.is_some(), "die gelöschte Verbindung bleibt weich gelöscht");
    let origin: (String, String, String) =
        sqlx::query_as("SELECT origin_kind, origin_name, created FROM idea_link WHERE uuid = ?")
            .bind(sample_uuid(151))
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(origin, ("persona".into(), "@lektor".into(), "2026-01-07T20:00:00.000Z".into()), "Herkunft und Zeit");

    let positions: Vec<(String, i64, f64, f64)> =
        sqlx::query_as("SELECT uuid, idea_id, x, y FROM idea_map_pos WHERE work_id = ? ORDER BY uuid")
            .bind(applied.work)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        positions,
        vec![(sample_uuid(152), seil, 120.5, 0.1 + 0.2), (sample_uuid(153), lampe, -1_000_000.0, -80.25)],
        "Lagen bit-genau mit ihren Kennungen"
    );
    let graves: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE kind IN ('idea_link', 'idea_map_pos') ORDER BY uuid",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        graves,
        vec![
            (sample_uuid(154), "idea_link".into(), Some(sample_uuid(1))),
            (sample_uuid(155), "idea_map_pos".into(), Some(sample_uuid(1)))
        ],
        "die Grabsteine der Karte kommen mit, beide mit der Werk-UUID"
    );
}

#[tokio::test]
async fn dumping_after_a_new_import_gives_back_the_same_map_and_tombstones() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    let work = import_new(&store, &doc).await;
    let dumped = store.exchange_dump(work).await.unwrap();
    // Ohne Verlust: dieselben Verbindungen (auch die gelöschte) und Lagen in derselben Reihenfolge.
    assert_eq!(dumped.map, doc.work.map, "Karte unverändert zurück");
    for kind in [ExchangeTombstoneKind::Link, ExchangeTombstoneKind::Position] {
        assert!(dumped.deleted.iter().any(|t| t.kind == kind), "Grabstein der Art {kind:?} in der Ausgabe");
    }
    // Eine Verbindung zu einem Einfall eines anderen Werks gehört nicht zur Ausgabe (beide Einfälle im Werk).
    let other: i64 = sqlx::query_scalar("SELECT id FROM work WHERE id <> ? ORDER BY id LIMIT 1")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let foreign: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, note, origin_kind, origin_name) VALUES (?, 'fremd', 'human', 'Probe') RETURNING id",
    )
    .bind(other)
    .fetch_one(store.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) VALUES (?, ?, 'leads_to', 'human', 'Probe')")
        .bind(idea_id(&store, 40).await)
        .bind(foreign)
        .execute(store.pool())
        .await
        .unwrap();
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.map.links.len(), 2, "die werkübergreifende Verbindung fehlt in der Ausgabe");
    baruch_exchange::read::validate(&baruch_exchange::ExchangeDocument {
        version: baruch_exchange::FORMAT_VERSION,
        exported: "2026-01-10T10:00:00.000Z".into(),
        instance: doc.instance.clone(),
        generator: "Probe".into(),
        work: dumped,
        unknown: Vec::new(),
    })
    .expect("die Ausgabe ist ein gültiges Dokument");
}

#[tokio::test]
async fn merging_the_same_file_again_adds_no_link_or_position() {
    let (_dir, store) = open().await;
    let doc = sample_document();
    import_new(&store, &doc).await;
    let before = (count(&store, "idea_link").await, count(&store, "idea_map_pos").await);
    let report = merge(&store, &doc).await;
    assert_eq!((report.add.links, report.add.positions), (0, 0), "nichts doppelt: {:?}", report.items);
    assert_eq!(report.add.total(), 0, "auch sonst nichts");
    assert_eq!((count(&store, "idea_link").await, count(&store, "idea_map_pos").await), before, "keine Zeile mehr");
}

#[tokio::test]
async fn merging_adds_a_new_link_but_never_doubles_a_pair_and_never_overwrites() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Auf der anderen Seite: eine neue Verbindung, eine geänderte Notiz an 150 und „widerspricht“ zwischen 40 und 41
    // andersherum aufgeschrieben (neu, denn 151 verbindet 41 und 42).
    doc.work.map.links.push(link(97, 42, 40, ExchangeLinkKind::BelongsTo));
    doc.work.map.links.push(link(99, 41, 40, ExchangeLinkKind::Contradicts));
    doc.work.map.links[0].note = "Dort geändert.".into();
    let preview = store.exchange_preview(&doc, SHA, None).await.unwrap();
    let report = merge(&store, &doc).await;
    assert_eq!(report.add, preview.add, "Vorschau und Übernehmen zählen gleich");
    assert_eq!(report.add.links, 2, "97 und 99 kommen dazu: {:?}", report.items);
    assert!(link_row(&store, &sample_uuid(97)).await.is_some(), "neue Verbindung mit ihrer Kennung");
    let contra = link_row(&store, &sample_uuid(99)).await.expect("99 angelegt");
    assert!(contra.0 < contra.1, "widerspricht auch beim Abgleich normalisiert");
    assert_eq!(
        link_row(&store, &sample_uuid(150)).await.unwrap().3,
        "Vom Seil zur Lampe.",
        "eine bekannte Verbindung wird nie überschrieben"
    );
    assert!(
        report.deviations.iter().any(|d| d.uuid == sample_uuid(150) && d.label.contains("Notiz weicht ab")),
        "die abweichende Notiz ist nur ein Hinweis: {:?}",
        report.deviations
    );
    // Eine dritte Instanz hat dieselbe Verbindung wie 150 (40 führt weiter 41) selbst angelegt, unter Kennung 98, und
    // „widerspricht“ 40/41 unter Kennung 92 andersherum: beides gibt es hier schon – kein Doppel.
    let mut third = sample_document();
    third.work.map.links[0].uuid = sample_uuid(98);
    third.work.map.links.push(link(92, 40, 41, ExchangeLinkKind::Contradicts));
    let report = merge(&store, &third).await;
    assert_eq!(report.add.links, 0, "dasselbe Paar mit derselben Art kommt nicht doppelt: {:?}", report.items);
    assert!(link_row(&store, &sample_uuid(98)).await.is_none(), "98 nicht angelegt");
    for skipped in [98, 92] {
        assert!(
            report.deviations.iter().any(|d| d.uuid == sample_uuid(skipped) && d.label.contains("anderer Kennung")),
            "das übersprungene Doppel {skipped} steht als Hinweis da: {:?}",
            report.deviations
        );
    }
    let links: i64 =
        sqlx::query_scalar("SELECT count(*) FROM idea_link l JOIN idea i ON i.id = l.from_id WHERE i.work_id = ?")
            .bind(work)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(links, 4, "zwei aus dem Einlesen, zwei aus dem Abgleich");
    let again = merge(&store, &doc).await;
    assert_eq!(again.add.total(), 0, "ein zweiter Abgleich legt nichts an");
}

#[tokio::test]
async fn tombstones_here_keep_links_and_positions_from_coming_back() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier hart gelöscht: Lage 152 („Anordnung zurücksetzen“) und Verbindung 150 (wie über die Kaskade).
    sqlx::query("DELETE FROM idea_map_pos WHERE uuid = ?").bind(sample_uuid(152)).execute(store.pool()).await.unwrap();
    sqlx::query("DELETE FROM idea_link WHERE uuid = ?").bind(sample_uuid(150)).execute(store.pool()).await.unwrap();
    let kinds: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT kind, work_uuid FROM uuid_tombstone WHERE uuid IN (?, ?) ORDER BY kind")
            .bind(sample_uuid(150))
            .bind(sample_uuid(152))
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        kinds,
        vec![("idea_link".into(), Some(sample_uuid(1))), ("idea_map_pos".into(), Some(sample_uuid(1)))],
        "Vorbedingung: die Trigger aus 0043 hinterlassen Grabsteine mit der Werk-UUID"
    );
    // Dazu eine neue Verbindung zu einem Einfall, den es hier nicht im Werk gibt.
    doc.work.ideas.push(baruch_exchange::ExchangeIdea {
        uuid: sample_uuid(43),
        words: vec!["Brunnen".into()],
        created: "2026-01-09T09:00:00.000Z".into(),
        updated: "2026-01-09T09:00:00.000Z".into(),
        ..Default::default()
    });
    doc.work.map.links.push(link(96, 40, 43, ExchangeLinkKind::ExampleOf));
    let report = merge(&store, &doc).await;
    let deleted: Vec<(&str, &str)> = report.deleted_here.iter().map(|d| (d.kind, d.uuid.as_str())).collect();
    assert!(
        deleted.contains(&("verbindung", sample_uuid(150).as_str())),
        "Verbindung 150 „hier gelöscht“: {deleted:?}"
    );
    assert!(deleted.contains(&("lage", sample_uuid(152).as_str())), "Lage 152 „hier gelöscht“: {deleted:?}");
    assert!(link_row(&store, &sample_uuid(150)).await.is_none(), "die Verbindung kommt nicht wieder");
    let back: i64 = sqlx::query_scalar("SELECT count(*) FROM idea_map_pos WHERE uuid = ?")
        .bind(sample_uuid(152))
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(back, 0, "die Lage kommt nicht wieder");
    // Einfall 43 kommt neu dazu, die Verbindung 96 also auch (beide Einfälle sind dann hier im Werk).
    assert!(link_row(&store, &sample_uuid(96)).await.is_some(), "Verbindung zum eben eingelesenen Einfall");
    let _ = work;
}

#[tokio::test]
async fn a_link_to_an_idea_outside_the_work_here_is_only_a_hint() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Einfall 41 gehört hier inzwischen zu einem anderen Werk (umgezogen); eine neue Verbindung 40 → 41 geht dann
    // nicht, denn Verbindungen gibt es nur innerhalb eines Werks.
    let other: i64 = sqlx::query_scalar("SELECT id FROM work WHERE id <> ? ORDER BY id LIMIT 1")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE idea SET work_id = ? WHERE uuid = ?")
        .bind(other)
        .bind(sample_uuid(41))
        .execute(store.pool())
        .await
        .unwrap();
    doc.work.map.links.push(link(95, 40, 41, ExchangeLinkKind::BelongsTo));
    let before = count(&store, "idea_link").await;
    let report = merge(&store, &doc).await;
    assert_eq!(report.add.links, 0, "keine Verbindung über die Werkgrenze: {:?}", report.items);
    assert_eq!(count(&store, "idea_link").await, before, "nichts geschrieben");
    assert!(
        report.deviations.iter().any(|d| d.uuid == sample_uuid(95) && d.label.contains("fehlt hier")),
        "Hinweis, dass ein Einfall fehlt: {:?}",
        report.deviations
    );
}

#[tokio::test]
async fn the_local_position_wins_when_merging() {
    let (_dir, store) = open().await;
    let mut doc = sample_document();
    let work = import_new(&store, &doc).await;
    // Hier verschoben (gleiche Kennung) und dort unter neuer Kennung für denselben Einfall gesetzt.
    sqlx::query("UPDATE idea_map_pos SET x = 7.0, y = 8.0 WHERE uuid = ?")
        .bind(sample_uuid(152))
        .execute(store.pool())
        .await
        .unwrap();
    doc.work.map.positions[0].x = 999.0;
    doc.work.map.positions[1] = ExchangePosition {
        uuid: sample_uuid(94),
        idea: sample_uuid(41),
        x: 1.0,
        y: 2.0,
        updated: "2026-01-09T12:00:00.000Z".into(),
    };
    // Und eine Lage für den gelöschten Einfall 42, der hier noch keine hat: die kommt.
    doc.work.map.positions.push(ExchangePosition {
        uuid: sample_uuid(93),
        idea: sample_uuid(42),
        x: -3.0,
        y: 4.5,
        updated: "2026-01-09T12:00:00.000Z".into(),
    });
    let report = merge(&store, &doc).await;
    assert_eq!(report.add.positions, 1, "nur die Lage ohne Gegenstück kommt: {:?}", report.items);
    let rows: Vec<(String, f64, f64)> =
        sqlx::query_as("SELECT uuid, x, y FROM idea_map_pos WHERE work_id = ? ORDER BY id")
            .bind(work)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![(sample_uuid(152), 7.0, 8.0), (sample_uuid(153), -1_000_000.0, -80.25), (sample_uuid(93), -3.0, 4.5)],
        "lokal gewinnt: die eigene Lage bleibt, die fremde für denselben Einfall kommt nicht"
    );
}
