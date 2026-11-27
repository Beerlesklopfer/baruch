// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 38: the work language in the store – `work_language`, `set_work_language`, and the field `language` in
//! `WorkSummary` and `WorkMeta`. Real SQLite file in a temporary directory, invented works.

use baruch_core::WorkKind;
use baruch_store::{NewWork, Preset, Store, StoreError, WorkId, WorkLanguage};

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn create(store: &Store, title: &str, kind: WorkKind) -> WorkId {
    let preset = if kind == WorkKind::Sermon { Preset::Sermon } else { Preset::Book };
    let new = NewWork { title, author: "Tilda Rohr", kind, preset, subtitle: "", audience: "", bible: "" };
    store.create_work(&new, "Autor").await.unwrap().work
}

#[tokio::test]
async fn summary_and_meta_carry_the_language_of_each_work() {
    let (_dir, store) = open().await;
    let ferry = create(&store, "The Ferry at Dusk", WorkKind::Book).await;
    let salt = create(&store, "Salz im Brot", WorkKind::Sermon).await;
    assert_eq!(
        store.work_meta(ferry).await.unwrap().language,
        WorkLanguage::De,
        "a new work starts German (default of migration 0055)"
    );
    assert!(store.set_work_language(ferry, WorkLanguage::En).await.unwrap(), "German → English is a change");

    let meta = store.work_meta(ferry).await.unwrap();
    assert_eq!(meta.language, WorkLanguage::En, "work_meta reads the stored language");
    assert!(meta.updated.is_some(), "a changed language counts as a change of the work data (work.updated)");
    assert_eq!(
        store.work_meta(salt).await.unwrap().language,
        WorkLanguage::De,
        "the language is per work: the other work stays German"
    );

    let list = store.work_list().await.unwrap();
    let of = |id: WorkId| list.iter().find(|w| w.id == id).map(|w| w.language);
    assert_eq!(of(ferry), Some(WorkLanguage::En), "work_list carries the language of the English work");
    assert_eq!(of(salt), Some(WorkLanguage::De), "and of the German work");

    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["language"], "en", "the JSON field is `language` with the code, as RPC work.meta returns it");
    let json = serde_json::to_value(list.iter().find(|w| w.id == salt).unwrap()).unwrap();
    assert_eq!(json["language"], "de", "also in the summary of work.list");
}

#[tokio::test]
async fn setting_the_language_touches_nothing_else_and_makes_no_commit() {
    let (dir, store) = open().await;
    let work = create(&store, "The Ferry at Dusk", WorkKind::Book).await;
    let before = store.load_work(work).await.unwrap();
    let meta_before = store.work_meta(work).await.unwrap();
    let mirror = dir.path().join("works");
    let commits =
        |path: &std::path::Path| -> usize { std::fs::read_dir(path).map(|entries| entries.count()).unwrap_or(0) };
    let mirror_entries = commits(&mirror);

    assert!(store.set_work_language(work, WorkLanguage::En).await.unwrap(), "the language changes");
    assert!(!store.set_work_language(work, WorkLanguage::En).await.unwrap(), "the same language again is no change");

    let after = store.load_work(work).await.unwrap();
    assert_eq!(after.nodes.len(), before.nodes.len(), "the outline is untouched");
    for (a, b) in after.nodes.iter().zip(&before.nodes) {
        let bodies = |node: &baruch_core::Node| -> Vec<(u32, String)> {
            node.revisions.iter().map(|r| (r.number, r.body.clone())).collect()
        };
        assert_eq!(bodies(a), bodies(b), "no revision is created or changed: the language is not book text");
    }
    let meta_after = store.work_meta(work).await.unwrap();
    assert_eq!(
        (&meta_after.title, &meta_after.slug, &meta_after.template, meta_after.template_rev, &meta_after.uuid),
        (&meta_before.title, &meta_before.slug, &meta_before.template, meta_before.template_rev, &meta_before.uuid),
        "title, slug, template and uuid stay as they were"
    );
    assert_eq!(
        commits(&mirror),
        mirror_entries,
        "the store writes nothing into the mirror directory (like work_timing)"
    );
    assert!(
        matches!(store.set_work_language(work + 1000, WorkLanguage::De).await, Err(StoreError::UnknownWork(_))),
        "an unknown work is an error, nothing is created"
    );
}
