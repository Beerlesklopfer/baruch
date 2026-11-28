// SPDX-License-Identifier: AGPL-3.0-or-later
//! Book details and cover in the store (migration 0017, Round 11, task cover): defaults without a row, limits of
//! the fields, ISBN with check digit, cover only from the same work, deleting image and work.

use baruch_core::WorkKind;
use baruch_store::{BookEdit, NewAsset, NewWork, Preset, Store, StoreError, TitleArea, WorkId};

const LEAF: &[u8] =
    b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"6\"><rect width=\"4\" height=\"6\"/></svg>";

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn work(store: &Store, title: &str) -> WorkId {
    let new = NewWork {
        title,
        author: "Erika Mustermann",
        kind: WorkKind::Book,
        preset: Preset::Book,
        subtitle: "",
        audience: "",
        bible: "",
        language: Default::default(),
    };
    store.create_work(&new, "Autor").await.unwrap().work
}

async fn image(store: &Store, work: WorkId, file_name: &str) -> String {
    let new = NewAsset {
        file_name,
        name: None,
        bytes: LEAF,
        as_base64: false,
        path: None,
        caption: "Blatt",
        alt: "Ein Blatt",
        note: "",
    };
    store.add_asset(work, &new, "Autor").await.unwrap().name
}

#[tokio::test]
async fn a_work_without_a_row_has_the_defaults() {
    let (_dir, store) = open().await;
    let w = work(&store, "Vorgaben").await;
    let meta = store.book_meta(w).await.unwrap();
    assert_eq!(meta.cover, None, "kein Umschlag");
    assert_eq!(meta.title_area, TitleArea::Top, "Titel oben");
    assert!(!meta.front_matter && meta.cover_pages, "ohne Titelei, Umschlagseiten an");
    assert_eq!((meta.paper_mm, meta.bleed_mm), (0.1, 3.0), "0,1 mm Papier, 3 mm Beschnitt");
    assert!(matches!(store.book_meta(9999).await, Err(StoreError::UnknownWork(9999))), "unbekanntes Werk");
}

#[tokio::test]
async fn book_meta_is_checked_and_only_changes_report_fields() {
    let (_dir, store) = open().await;
    let w = work(&store, "Grenzen").await;
    let photo = image(&store, w, "foto.svg").await;
    let edit = BookEdit {
        publisher: Some("  Beetverlag ".into()),
        blurb: Some("Erste Zeile\r\nzweite Zeile".into()),
        isbn: Some("978 3 16 148410 0".into()),
        year: Some("2026".into()),
        author_photo: Some(Some(photo.clone())),
        title_area: Some(TitleArea::Bottom),
        front_matter: Some(true),
        paper_mm: Some(0.08),
        ..BookEdit::default()
    };
    let change = store.set_book_meta(w, &edit).await.unwrap();
    assert_eq!(
        change.changed,
        ["publisher", "year", "isbn", "blurb", "author_photo", "title_area", "front_matter", "paper_mm"],
        "geänderte Felder in fester Reihenfolge"
    );
    assert_eq!(change.after.publisher, "Beetverlag", "getrimmt");
    assert_eq!(change.after.blurb, "Erste Zeile\nzweite Zeile", "CRLF → LF, mehrzeilig erlaubt");
    assert_eq!(change.after.author_photo.as_deref(), Some(photo.as_str()), "Foto über den Bildnamen");
    assert!(change.after.updated.is_some(), "Zeitpunkt der Änderung");
    assert_eq!(change.before.publisher, "", "vorher leer (für den Commit-Rumpf)");
    let same = store.set_book_meta(w, &edit).await.unwrap();
    assert!(same.changed.is_empty(), "gleiche Werte: keine Änderung");
    // Round 20c: an ISBN that is only written differently (hyphens) is a change and is stored.
    let again =
        store.set_book_meta(w, &BookEdit { isbn: Some("978-3-16-148410-0".into()), ..edit.clone() }).await.unwrap();
    assert_eq!(again.changed, ["isbn"], "ISBN nur anders geschrieben: die Schreibweise wird gespeichert");
    assert_eq!(again.after.isbn, "978-3-16-148410-0", "gespeichert wie eingegeben");

    for (bad, why) in [
        (BookEdit { publisher: Some("a".repeat(201)), ..BookEdit::default() }, "einzeilig höchstens 200 Zeichen"),
        (BookEdit { place: Some("zwei\nZeilen".into()), ..BookEdit::default() }, "einzeilig ohne Zeilenumbruch"),
        (BookEdit { blurb: Some("x".repeat(4001)), ..BookEdit::default() }, "Klappentext höchstens 4000"),
        (BookEdit { bio: Some("x".repeat(2001)), ..BookEdit::default() }, "Kurzbiografie höchstens 2000"),
        (BookEdit { dedication: Some("x".repeat(1001)), ..BookEdit::default() }, "Widmung höchstens 1000"),
        (BookEdit { year: Some("226".into()), ..BookEdit::default() }, "Jahr vier Ziffern"),
        (BookEdit { isbn: Some("978-3-16-148410-9".into()), ..BookEdit::default() }, "falsche Prüfziffer"),
        (BookEdit { isbn: Some("3-16-148410-X".into()), ..BookEdit::default() }, "ISBN-10 ist keine ISBN-13"),
        (BookEdit { paper_mm: Some(0.0), ..BookEdit::default() }, "Papier dicker als 0"),
        (BookEdit { paper_mm: Some(f64::NAN), ..BookEdit::default() }, "Papier keine Zahl"),
        (BookEdit { bleed_mm: Some(10.5), ..BookEdit::default() }, "Beschnitt höchstens 10"),
    ] {
        let mut bad = bad;
        bad.edition = Some("darf nicht ankommen".into());
        assert!(matches!(store.set_book_meta(w, &bad).await, Err(StoreError::Refused(_))), "abgelehnt: {why}");
    }
    let unknown = BookEdit { author_photo: Some(Some("fehlt.svg".into())), ..BookEdit::default() };
    assert!(matches!(store.set_book_meta(w, &unknown).await, Err(StoreError::UnknownAsset { .. })), "unbekanntes Foto");
    assert_eq!(store.book_meta(w).await.unwrap().edition, "", "abgelehnte Änderungen schreiben nichts");
    let clear = BookEdit { author_photo: Some(None), ..BookEdit::default() };
    assert_eq!(store.set_book_meta(w, &clear).await.unwrap().after.author_photo, None, "Foto entfernt");
}

#[tokio::test]
async fn the_cover_comes_from_the_same_work_and_disappears_with_its_image() {
    let (_dir, store) = open().await;
    let w = work(&store, "Mit Umschlag").await;
    let other = work(&store, "Anderes Werk").await;
    let leaf = image(&store, w, "blatt.svg").await;
    let foreign = image(&store, other, "fremd.svg").await;

    let change = store.set_work_cover(w, Some(&leaf)).await.unwrap();
    assert_eq!((change.before, change.after.as_deref()), (None, Some(leaf.as_str())), "gesetzt");
    assert_eq!(store.work_cover(w).await.unwrap().as_deref(), Some(leaf.as_str()), "gelesen");
    let same = store.set_work_cover(w, Some(&leaf)).await.unwrap();
    assert_eq!(same.before, same.after, "noch einmal: keine Änderung");
    assert!(
        matches!(store.set_work_cover(w, Some(&foreign)).await, Err(StoreError::UnknownAsset { .. })),
        "ein Bild eines anderen Werks gibt es in diesem Werk nicht"
    );
    // Even bypassing Rust the trigger prevents a foreign cover.
    let foreign_id: i64 =
        sqlx::query_scalar("SELECT id FROM asset WHERE work_id = ?").bind(other).fetch_one(store.pool()).await.unwrap();
    let raw = sqlx::query("UPDATE work_book SET cover_asset_id = ? WHERE work_id = ?")
        .bind(foreign_id)
        .bind(w)
        .execute(store.pool())
        .await;
    assert!(raw.unwrap_err().to_string().contains("desselben Werks"), "Trigger lehnt den fremden Umschlag ab");

    store.delete_asset(w, &leaf, "Autor").await.unwrap();
    assert_eq!(store.work_cover(w).await.unwrap(), None, "Bild gelöscht → kein Umschlag mehr (ON DELETE SET NULL)");
    let leaf = image(&store, w, "blatt2.svg").await;
    store.set_work_cover(w, Some(&leaf)).await.unwrap();
    store.delete_work(w, "Mit Umschlag").await.expect("Werk mit Umschlag und Buchangaben lässt sich löschen");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM work_book WHERE work_id = ?")
        .bind(w)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0, "die Zeile geht mit dem Werk");
    assert_eq!(store.set_work_cover(other, None).await.unwrap().after, None, "Entfernen ohne Umschlag schadet nicht");
}
