// SPDX-License-Identifier: AGPL-3.0-or-later
//! The LanceDB index in the temp folder: writing, replacing, scopes, search with cosine distance and
//! `baruch-index.json`. Vectors are chosen by hand (no embedding needed).

use baruch_lancedb::{Entry, Index, IndexMeta, Scope};
use serde_json::json;

fn entry(id: &str, work: Option<i64>, kind: &str, text: &str) -> Entry {
    Entry {
        id: id.into(),
        work,
        kind: kind.into(),
        document: work.map_or_else(|| "title:1".to_owned(), |w| format!("work:{w}")),
        section: Some(format!("key:{id}")),
        title: format!("Titel {id}"),
        text: text.into(),
        meta: json!({ "id": id }),
    }
}

#[tokio::test]
async fn rows_are_written_replaced_counted_and_found_by_cosine_distance() {
    let dir = tempfile::tempdir().unwrap();
    let index = Index::open(&dir.path().join("lancedb")).await.unwrap();
    assert!(!index.exists().await.unwrap(), "neu: noch keine Tabelle");
    assert_eq!(index.count(None).await.unwrap(), 0, "leer");
    assert!(
        index.search(vec![1.0, 0.0, 0.0], Scope::Visible(1), vec![], 5).await.unwrap().is_empty(),
        "Suche ohne Tabelle: keine Treffer, kein Fehler"
    );
    let rows = [
        (entry("a", Some(1), "section", "Leuchtturm"), vec![1.0, 0.0, 0.0]),
        (entry("b", Some(1), "idea", "Senfkorn"), vec![0.0, 1.0, 0.0]),
        (entry("c", Some(2), "section", "anderes Werk"), vec![1.0, 0.0, 0.0]),
        (entry("d", None, "library", "Bibliothek"), vec![0.7, 0.7, 0.0]),
    ];
    index.replace(Scope::Visible(1), vec![], rows[..2].to_vec(), 3).await.unwrap();
    index.replace(Scope::Work(2), vec![], rows[2..3].to_vec(), 3).await.unwrap();
    index.replace(Scope::Shared, vec![], rows[3..].to_vec(), 3).await.unwrap();
    assert_eq!(index.count(None).await.unwrap(), 4, "vier Zeilen");
    assert_eq!(index.count(Some(Scope::Work(1))).await.unwrap(), 2, "zwei in Werk 1");
    assert_eq!(index.count(Some(Scope::Visible(1))).await.unwrap(), 3, "Werk 1 sieht seine und die gemeinsamen");

    let found = index.search(vec![1.0, 0.0, 0.0], Scope::Visible(1), vec![], 10).await.unwrap();
    let ids: Vec<&str> = found.iter().map(|f| f.entry.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "d", "b"], "nach Abstand; Werk 2 bleibt draußen");
    assert!((found[0].score() - 1.0).abs() < 1e-4, "gleiche Richtung: Ähnlichkeit 1, nicht {}", found[0].score());
    assert!(
        (found[1].score() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3,
        "45°: cos = 0,707, nicht {}",
        found[1].score()
    );
    assert!(found[2].score().abs() < 1e-4, "senkrecht: 0, nicht {}", found[2].score());
    assert_eq!(found[0].entry, entry("a", Some(1), "section", "Leuchtturm"), "Zeile kommt vollständig zurück");

    let ideas = index.search(vec![1.0, 0.0, 0.0], Scope::Visible(1), vec!["idea".into()], 10).await.unwrap();
    assert_eq!(ideas.len(), 1, "nur die Art idea: {ideas:?}");

    // If a row changes the work, every scope clears its part (Round 19a: deleting only within the scope).
    index.replace(Scope::Work(1), vec!["b".into()], vec![], 3).await.unwrap();
    index
        .replace(
            Scope::Work(2),
            vec!["b".into()],
            vec![(entry("b", Some(2), "idea", "Senfkorn neu"), vec![0.0, 1.0, 0.0])],
            3,
        )
        .await
        .unwrap();
    assert_eq!(index.count(Some(Scope::Work(1))).await.unwrap(), 1, "b ist aus Werk 1 verschwunden");
    assert_eq!(index.count(Some(Scope::Work(2))).await.unwrap(), 2, "b ist jetzt in Werk 2");
    let stored = index.stored(Scope::Work(2)).await.unwrap();
    let b = stored.iter().find(|s| s.id == "b").expect("b gespeichert");
    assert_eq!(b.vector, vec![0.0, 1.0, 0.0], "Vektor kommt unverändert zurück");
    assert_eq!(b.content_hash, entry("b", Some(2), "idea", "Senfkorn neu").content_hash(), "Inhalts-Hash");
    assert_eq!(b.row_hash, entry("b", Some(2), "idea", "Senfkorn neu").row_hash(), "Zeilen-Hash");

    index.delete_scope(Scope::OtherWorks(vec![1])).await.unwrap();
    assert_eq!(index.count(None).await.unwrap(), 2, "Werk 2 verworfen, Werk 1 und gemeinsam bleiben");
    assert!(
        index.replace(Scope::Work(1), vec![], vec![(entry("e", Some(1), "idea", "x"), vec![1.0])], 3).await.is_err(),
        "falsche Dimension"
    );
}

#[tokio::test]
async fn the_meta_file_holds_model_and_dimension_and_reset_drops_everything() {
    let dir = tempfile::tempdir().unwrap();
    let index = Index::open(dir.path()).await.unwrap();
    assert_eq!(index.meta().unwrap().dimension, None, "ohne Datei: kein Modell, keine Dimension");
    let meta = IndexMeta {
        layout: baruch_lancedb::LAYOUT,
        requested: Some("text-embedding-nomic-embed-text-v1.5".into()),
        model: Some("text-embedding-nomic-embed-text-v1.5".into()),
        dimension: Some(3),
        built: [("work:1".to_owned(), "2026-11-07T10:00:00.000Z".to_owned())].into(),
        document_prefix: Some("search_document: ".into()),
    };
    index.set_meta(&meta).unwrap();
    assert_eq!(index.meta().unwrap(), meta, "zurückgelesen wie geschrieben");
    assert!(dir.path().join("baruch-index.json").exists(), "Datei im Ordner des Index");
    index
        .replace(Scope::Work(1), vec![], vec![(entry("a", Some(1), "section", "x"), vec![1.0, 0.0, 0.0])], 3)
        .await
        .unwrap();
    index.reset().await.unwrap();
    assert!(!index.exists().await.unwrap(), "Tabelle verworfen");
    assert_eq!(index.meta().unwrap().model, None, "Modell vergessen");
    std::fs::write(dir.path().join("baruch-index.json"), "{kaputt").unwrap();
    assert!(index.meta().is_err(), "beschädigte Datei ist ein ehrlicher Fehler");
}

#[tokio::test]
async fn replace_deletes_only_inside_its_scope_and_refuses_rows_outside_it() {
    let dir = tempfile::tempdir().unwrap();
    let index = Index::open(dir.path()).await.unwrap();
    // Same identifier in two works: `node.key` or `node.id` can be assigned again after a deletion.
    index
        .replace(
            Scope::Work(1),
            vec![],
            vec![(entry("section:7:1", Some(1), "section", "Werk eins"), vec![1.0, 0.0])],
            2,
        )
        .await
        .unwrap();
    index
        .replace(
            Scope::Work(2),
            vec![],
            vec![(entry("section:7:1", Some(2), "section", "Werk zwei"), vec![0.0, 1.0])],
            2,
        )
        .await
        .unwrap();
    assert_eq!(index.count(None).await.unwrap(), 2, "zwei Zeilen mit gleicher Kennung, je Werk eine");

    index
        .replace(
            Scope::Work(1),
            vec!["section:7:1".into()],
            vec![(entry("section:7:1", Some(1), "section", "Werk eins neu"), vec![1.0, 0.0])],
            2,
        )
        .await
        .unwrap();
    assert_eq!(
        index.count(Some(Scope::Work(2))).await.unwrap(),
        1,
        "Begründung: replace(Scope::Work(1), …) löscht nur `id IN (…) AND work = 1`; die frische Zeile von Werk 2 bleibt"
    );
    let two = index.stored(Scope::Work(2)).await.unwrap();
    assert_eq!(two[0].vector, vec![0.0, 1.0], "Werk 2 behält seinen eigenen Vektor");
    let one = index.stored(Scope::Work(1)).await.unwrap();
    assert_eq!(one.len(), 1, "Werk 1 hat genau die ersetzte Zeile: {one:?}");
    assert_eq!(
        one[0].content_hash,
        entry("x", None, "section", "Werk eins neu").content_hash(),
        "neuer Text in Werk 1"
    );

    let e = index
        .replace(Scope::Work(1), vec![], vec![(entry("idea:3", Some(2), "idea", "fremd"), vec![1.0, 0.0])], 2)
        .await
        .unwrap_err();
    assert_eq!(e.code(), "invalid", "Begründung: eine Zeile von Werk 2 gehört nicht in den Bereich Werk 1: {e}");
    let e = index
        .replace(
            Scope::Shared,
            vec!["section:7:1".into()],
            vec![(entry("idea:4", Some(1), "idea", "x"), vec![1.0, 0.0])],
            2,
        )
        .await
        .unwrap_err();
    assert_eq!(e.code(), "invalid", "auch der gemeinsame Bereich nimmt keine Zeile eines Werks: {e}");
    assert_eq!(index.count(None).await.unwrap(), 2, "eine abgelehnte Ersetzung ändert nichts, auch nicht das Löschen");

    index.replace(Scope::Shared, vec!["section:7:1".into()], vec![], 2).await.unwrap();
    assert_eq!(
        index.count(None).await.unwrap(),
        2,
        "im gemeinsamen Bereich gibt es die Kennung nicht: nichts gelöscht"
    );
}
