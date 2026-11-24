// SPDX-License-Identifier: AGPL-3.0-or-later
//! Archiv im Store (Runde 22, befund-rueckfrage): `Store::retired_works` listet gelöschte Werke, `Store::restore_retired`
//! legt eines mit dem reservierten slug aus dem Typst-Text des Archivordners wieder an. Echte SQLite-Datei im
//! Temp-Ordner; das Werk ist frei erfunden.

use baruch_core::{Author, RevisionStatus};
use baruch_store::{Store, StoreError, WorkId};

const LEUCHTTURM: &str = "\
#set document(title: \"Der Leuchtturmwärter\", author: \"Ida Brandt\")
#set heading(numbering: \"1.\")

= Die Lampe

Jeden Abend steigt Jaro die Wendeltreppe hinauf.

== Das Öl

Das Öl riecht nach Tran und Winter.

= Der Sturm

In der dritten Nacht reißt die Kette der Boje.
";

const TITLE: &str = "Der Leuchtturmwärter";

/// Aktuelle Texte in Baumreihenfolge.
async fn current_texts(store: &Store, work: WorkId) -> Vec<String> {
    let loaded = store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| n.current_index().map(|i| n.revisions[i].body.clone()).unwrap_or_default()).collect()
}

/// Werk anlegen, eine Fassung ändern (damit nicht nur der Import zurückkommt), exportieren und löschen.
async fn deleted_lighthouse() -> (tempfile::TempDir, Store, String, String, Vec<String>, String) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let imported = store.import_typst(LEUCHTTURM, "leuchtturm.typ", "Ida").await.unwrap();
    store
        .save_human(
            imported.work,
            2,
            "= Der Sturm\n\nIn der dritten Nacht reißt die Kette. Jaro rudert hinaus.\n",
            "Ida",
        )
        .await
        .unwrap()
        .expect("geänderter Text ergibt eine neue Fassung");
    let texts = current_texts(&store, imported.work).await;
    let template = store.work_template(imported.work).await.unwrap();
    let export = store.export_typst(imported.work).await.unwrap();
    store.delete_work(imported.work, TITLE).await.unwrap();
    (dir, store, imported.slug, export, texts, template)
}

#[tokio::test]
async fn retired_works_lists_deleted_works_newest_first() {
    let (_dir, store, slug, ..) = deleted_lighthouse().await;
    let retired = store.retired_works().await.unwrap();
    let entry = retired.iter().find(|r| r.slug == slug).expect("das gelöschte Werk steht im Archiv");
    assert_eq!(entry.title, TITLE, "Titel aus work_retired");
    assert!(!entry.deleted.is_empty(), "Zeitpunkt des Löschens");
}

#[tokio::test]
async fn a_retired_work_comes_back_with_its_slug_title_and_current_texts_as_version_one() {
    let (_dir, store, slug, export, texts, template) = deleted_lighthouse().await;
    let restored = store.restore_retired(&slug, &export, "Ida").await.unwrap();
    assert_eq!(restored.slug, slug, "der reservierte Ordnername gehört wieder dem Werk");
    assert_eq!(restored.title, TITLE, "der Titel aus work_retired");
    assert_eq!(restored.nodes, 3, "drei Gliederungspunkte wie vor dem Löschen");
    assert_eq!(store.work_slug(restored.work).await.unwrap(), slug, "slug in der Datenbank");
    assert_eq!(current_texts(&store, restored.work).await, texts, "gleiche aktuelle Texte wie vor dem Löschen");
    assert_eq!(store.work_template(restored.work).await.unwrap(), template, "die Werkvorlage kommt mit");
    let loaded = store.load_work(restored.work).await.unwrap();
    for node in &loaded.nodes {
        assert_eq!(node.revisions.len(), 1, "{}: nur Fassung 1 (ältere Fassungen bleiben im Archiv)", node.title);
        let rev = &node.revisions[0];
        assert_eq!(rev.number, 1, "{}: Fassung 1", node.title);
        assert_eq!(rev.status, RevisionStatus::Current, "{}: aktuell", node.title);
        assert_eq!(rev.author, Author::Human { name: "Ida".into() }, "{}: vom Menschen", node.title);
        assert_eq!(rev.note, "aus dem Archiv wiederhergestellt", "{}: Notiz nach Vertrag §2.16", node.title);
    }
    assert!(!store.retired_works().await.unwrap().iter().any(|r| r.slug == slug), "die Zeile in work_retired entfällt");
}

#[tokio::test]
async fn an_unknown_slug_is_refused_in_german_and_changes_nothing() {
    let (_dir, store, slug, export, ..) = deleted_lighthouse().await;
    let before = store.work_list().await.unwrap().len();
    let err = store.restore_retired("gibt-es-nicht", &export, "Ida").await.unwrap_err();
    assert!(
        matches!(&err, StoreError::Refused(m) if m.contains("nicht im Archiv")),
        "unbekannter Ordnername → Refused mit deutscher Meldung: {err}"
    );
    assert_eq!(store.work_list().await.unwrap().len(), before, "kein Werk angelegt");
    assert!(store.retired_works().await.unwrap().iter().any(|r| r.slug == slug), "das Archiv bleibt");
}

#[tokio::test]
async fn a_live_work_with_the_same_title_blocks_the_restore() {
    let (_dir, store, slug, export, ..) = deleted_lighthouse().await;
    let other = "= Anderes\n\nEin anderer Text mit demselben Titel.\n";
    store.import_typst_titled(other, "anders.typ", "Ida", "Import", Some(TITLE)).await.unwrap();
    let before = store.work_list().await.unwrap().len();
    let err = store.restore_retired(&slug, &export, "Ida").await.unwrap_err();
    assert!(
        matches!(&err, StoreError::Refused(m) if m.contains("denselben Titel")),
        "Titel vergeben → Refused mit deutscher Meldung: {err}"
    );
    assert_eq!(store.work_list().await.unwrap().len(), before, "nichts angelegt");
    assert!(
        store.retired_works().await.unwrap().iter().any(|r| r.slug == slug),
        "die Zeile in work_retired bleibt, damit es nach dem Umbenennen noch geht"
    );
}

#[tokio::test]
async fn a_text_without_headings_rolls_back_everything() {
    let (_dir, store, slug, ..) = deleted_lighthouse().await;
    let err = store.restore_retired(&slug, "Nur Fließtext ohne Überschrift.\n", "Ida").await.unwrap_err();
    assert!(matches!(err, StoreError::Import(_)), "keine Überschrift → Import-Fehler: {err}");
    assert!(
        store.retired_works().await.unwrap().iter().any(|r| r.slug == slug),
        "eine Transaktion: das Löschen der work_retired-Zeile ist zurückgerollt"
    );
    assert!(store.work_list().await.unwrap().iter().all(|w| w.slug != slug), "kein halbes Werk");
}
