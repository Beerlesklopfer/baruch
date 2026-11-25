// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 (mindmap_daten, Vertrag §1): Austauschformat Unterversion 9 im Store – Titel, Elternknoten, Reihenfolge
//! und Hauptthema der Einfälle neu einlesen und zurück ausgeben; Einlesen in zwei Schritten (der Elternknoten darf in
//! der Datei hinter dem Kind stehen); unbekannte bzw. gelöschte Elternknoten und Kreise lösen den Einfall mit
//! Hinweis im Bericht; eine Datei der Unterversion 8 lässt sich weiter einlesen; Abgleichen übernimmt den Baum neuer
//! Einfälle und meldet einen abweichenden Titel. Erfundenes Beispielwerk aus `baruch_exchange::sample`; echte
//! SQLite-Datei im Temp-Ordner; jede Zusicherung mit Begründung.

use baruch_exchange::read::read_xml;
use baruch_exchange::sample::{sample_document_u8, sample_document_u9, sample_uuid};
use baruch_exchange::write::{ImageMode, write_xml};
use baruch_exchange::{ExchangeDocument, ExchangeIdea, ExchangeIdeaStatus, ExchangeOriginKind, FORMAT_VERSION, Limits};
use baruch_store::exchange_apply::{ApplyMode, DeviationAction};
use baruch_store::{IdeaFilter, IdeaScope, Store, WorkId};

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000034";

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

/// (Titel, uuid des Elternknotens, position, main_topic) je Einfall des Werks, nach uuid.
async fn tree_rows(store: &Store, work: WorkId) -> Vec<(String, String, Option<String>, i64, i64)> {
    sqlx::query_as(
        "SELECT i.uuid, i.title, (SELECT p.uuid FROM idea p WHERE p.id = i.parent_id), i.position, i.main_topic \
         FROM idea i WHERE i.work_id = ? ORDER BY i.uuid",
    )
    .bind(work)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

fn extra(n: u32, title: &str, parent: Option<u32>) -> ExchangeIdea {
    ExchangeIdea {
        uuid: sample_uuid(n),
        title: title.to_owned(),
        parent: parent.map(sample_uuid),
        origin_kind: ExchangeOriginKind::Human,
        origin_name: "Hanna Probe".into(),
        status: ExchangeIdeaStatus::Open,
        created: "2026-03-01T09:00:00.000Z".into(),
        updated: "2026-03-01T09:00:00.000Z".into(),
        ..ExchangeIdea::default()
    }
}

#[tokio::test]
async fn a_new_import_carries_title_parent_place_and_main_topic_and_dumps_them_back_exactly() {
    let (_dir, store) = open().await;
    let doc = sample_document_u9();
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let work = applied.work;
    assert_eq!(applied.report.add.ideas, 3, "alle drei Einfälle des Beispiels angelegt");
    let rows = tree_rows(&store, work).await;
    assert_eq!(
        rows,
        [
            (sample_uuid(40), "Das Seil & sein <Knoten>".to_owned(), None, 1, 0),
            (sample_uuid(41), "Lampe am Seil".to_owned(), Some(sample_uuid(40)), 2, 0),
            (sample_uuid(42), String::new(), None, 3, 1),
        ],
        "Titel, Elternknoten (über die Kennung aufgelöst), Platz und Hauptthema stehen in der Datenbank"
    );
    // Der Titel steht im Volltextindex (wie beim Anlegen über den Store).
    let found = store
        .idea_search("Lampe", &IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() })
        .await
        .unwrap();
    assert!(found.iter().any(|(idea, _)| idea.title == "Lampe am Seil"), "über den Titel auffindbar: {found:?}");

    // Zurück ausgeben: derselbe Baum, und über XML gelesen wieder derselbe.
    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.ideas.len(), 3, "auch der gelöschte Einfall wird ausgegeben");
    for (there, here) in doc.work.ideas.iter().zip(&dumped.ideas) {
        assert_eq!(
            (&here.uuid, &here.title, &here.parent, here.position, here.main_topic),
            (&there.uuid, &there.title, &there.parent, there.position, there.main_topic),
            "Einfall {}: Titel, Elternknoten, Platz und Hauptthema ohne Verlust zurück",
            there.uuid
        );
    }
    let xml = write_xml(&ExchangeDocument { work: dumped.clone(), ..doc.clone() }, ImageMode::Inline).unwrap();
    assert!(xml.contains(&format!("version=\"{FORMAT_VERSION}\"")) && FORMAT_VERSION == 9, "Ausgabe in Unterversion 9");
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(back.work.ideas, dumped.ideas, "die Rundreise über XML erhält die Einfälle samt Baum");
}

#[tokio::test]
async fn a_parent_later_in_the_file_is_linked_and_unknown_deleted_or_circular_parents_are_loosened_with_a_hint() {
    let (_dir, store) = open().await;
    let mut doc = sample_document_u9();
    // 900 steht vor seinem Elternknoten 901 (zwei Schritte: erst Einfälle, dann Eltern); 902 nennt einen
    // Elternknoten, den es nicht gibt; 903 hängt unter dem gelöschten 42, ist selbst aber nicht gelöscht;
    // 904 und 905 nennen einander (Kreis).
    doc.work.ideas.push(extra(900, "Kind zuerst", Some(901)));
    doc.work.ideas.push(extra(901, "Elternknoten danach", None));
    doc.work.ideas.push(extra(902, "Waise", Some(4711)));
    doc.work.ideas.push(extra(903, "Unter Gelöschtem", Some(42)));
    doc.work.ideas.push(extra(904, "Kreis eins", Some(905)));
    doc.work.ideas.push(extra(905, "Kreis zwei", Some(904)));
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let rows = tree_rows(&store, applied.work).await;
    let parent_of = |n: u32| rows.iter().find(|r| r.0 == sample_uuid(n)).map(|r| r.2.clone()).unwrap();
    assert_eq!(parent_of(900), Some(sample_uuid(901)), "der Elternknoten steht später in der Datei und wird gefunden");
    assert_eq!(parent_of(902), None, "unbekannter Elternknoten: ohne Elternknoten übernommen");
    assert_eq!(parent_of(903), None, "gelöschter Elternknoten unter einem lebenden Einfall: gelöst");
    let circle = [parent_of(904), parent_of(905)];
    assert!(
        circle.iter().filter(|p| p.is_some()).count() <= 1,
        "ein Kreis wird nie gespeichert (höchstens eine der beiden Bindungen bleibt): {circle:?}"
    );
    let hints: Vec<&str> = applied
        .report
        .deviations
        .iter()
        .filter(|d| d.kind == "einfall" && d.action == DeviationAction::Hinweis)
        .map(|d| d.label.as_str())
        .collect();
    assert!(
        hints.iter().any(|h| h.contains("fehlt hier im Werk")),
        "der Bericht nennt den unbekannten Elternknoten: {hints:?}"
    );
    assert!(hints.iter().any(|h| h.contains("ist gelöscht")), "und den gelöschten: {hints:?}");
    assert!(hints.iter().any(|h| h.contains("Kreis")), "und den Kreis: {hints:?}");
    assert!(
        hints.iter().all(|h| h.contains("ohne Elternknoten übernommen")),
        "jeder Hinweis sagt, was geschah: {hints:?}"
    );
}

#[tokio::test]
async fn a_file_of_version_eight_is_still_read_and_its_ideas_get_the_defaults() {
    let (_dir, store) = open().await;
    // Eine Datei, wie Runde 22 sie schrieb: Unterversion 8, ohne die Attribute des Baums.
    let xml = write_xml(&sample_document_u8(), ImageMode::Inline).unwrap().replacen(
        &format!("version=\"{FORMAT_VERSION}\""),
        "version=\"8\"",
        1,
    );
    let doc = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(doc.version, 8, "Vorbedingung: die Datei trägt Unterversion 8");
    assert!(doc.unknown.is_empty(), "nichts Unbekanntes in einer echten Datei der Unterversion 8: {:?}", doc.unknown);
    let applied = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    let rows = tree_rows(&store, applied.work).await;
    assert_eq!(rows.len(), 3, "die Einfälle der alten Datei sind eingelesen");
    assert!(
        rows.iter().all(|(_, title, parent, position, main_topic)| title.is_empty()
            && parent.is_none()
            && *position == 0
            && *main_topic == 0),
        "fehlende Attribute = Vorgabe: kein Titel, kein Elternknoten, Platz 0, kein Hauptthema: {rows:?}"
    );
}

#[tokio::test]
async fn merging_adds_new_ideas_with_their_tree_and_reports_a_differing_title() {
    let (_dir, store) = open().await;
    let work = store.exchange_apply(&sample_document_u9(), SHA, ApplyMode::New, None).await.unwrap().work;
    let mut there = sample_document_u9();
    there.work.ideas[1].title = "Lampe am Tau".into();
    there.work.ideas.push(extra(910, "Docht", Some(41)));
    let applied = store.exchange_apply(&there, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.work, work, "Abgleich mit demselben Werk");
    assert_eq!(applied.report.add.ideas, 1, "ein neuer Einfall kommt dazu");
    let rows = tree_rows(&store, work).await;
    let docht = rows.iter().find(|r| r.0 == sample_uuid(910)).expect("der neue Einfall ist da");
    assert_eq!(
        docht.2,
        Some(sample_uuid(41)),
        "der neue Einfall hängt unter seinem (hier schon bekannten) Elternknoten"
    );
    let lamp = rows.iter().find(|r| r.0 == sample_uuid(41)).unwrap();
    assert_eq!(lamp.1, "Lampe am Seil", "der Titel des bekannten Einfalls bleibt (lokal gewinnt)");
    assert!(
        applied.report.deviations.iter().any(|d| d.kind == "einfall" && d.label.contains("Titel weicht ab")),
        "der abweichende Titel steht als Hinweis im Bericht: {:?}",
        applied.report.deviations.iter().map(|d| &d.label).collect::<Vec<_>>()
    );
}
