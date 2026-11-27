// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view settings in the store (Round 18c, migration 0044): without a row everything from the template, setting
//! and reading (normalised), taking back deletes the row, check of every field with a German message (nothing changed),
//! unknown work, no mirror (the values are never in the snapshot), a deleted work takes the row along. Works come
//! through `import_typst` (freely invented text).

use baruch_store::{MAX_SHEET_FONT_CHARS, SheetSettings, Store, StoreError};

const BOOK: &str = "#set document(title: \"Das Uferbuch\")\n\n= Steine\n\nDer Fluss rollt sie rund.\n";

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(BOOK, "ufer.typ", "Autor").await.unwrap().work;
    (dir, store, work)
}

async fn rows(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM work_sheet").fetch_one(store.pool()).await.unwrap()
}

fn full() -> SheetSettings {
    SheetSettings {
        font: Some("\"Erfundene Antiqua\", Georgia, serif".into()),
        heading_font: Some("'Probe Grotesk', sans-serif".into()),
        size: Some("11.5pt".into()),
        fill: Some("#222222".into()),
        heading_fill: Some("#7A1F1F".into()),
        background: Some("#FBF8F1".into()),
        width: Some("16cm".into()),
    }
}

#[tokio::test]
async fn without_a_row_everything_comes_from_the_template() {
    let (_dir, store, work) = fresh().await;
    let settings = store.sheet_settings(work).await.unwrap();
    assert!(settings.is_empty(), "ohne Zeile ist kein Feld gesetzt (alles aus der Vorlage): {settings:?}");
    assert_eq!(rows(&store).await, 0, "Lesen legt keine Zeile an");
}

#[tokio::test]
async fn settings_are_stored_normalized_replaced_and_cleared() {
    let (_dir, store, work) = fresh().await;
    store.set_sheet_settings(work, &full()).await.expect("gültige Einstellungen");
    let read = store.sheet_settings(work).await.unwrap();
    assert_eq!(read.heading_fill.as_deref(), Some("#7a1f1f"), "Farben werden klein geschrieben gespeichert");
    assert_eq!(read.background.as_deref(), Some("#fbf8f1"), "auch die Blattfarbe");
    assert_eq!(read.font, full().font, "die Schriftliste bleibt wörtlich");
    assert_eq!(read.width.as_deref(), Some("16cm"), "die Satzbreite bleibt");
    assert_eq!(rows(&store).await, 1, "eine Zeile je Werk");

    let fewer = SheetSettings { size: Some(" 12PT ".into()), ..SheetSettings::default() };
    store.set_sheet_settings(work, &fewer).await.unwrap();
    let read = store.sheet_settings(work).await.unwrap();
    assert_eq!(
        read,
        SheetSettings { size: Some("12pt".into()), ..SheetSettings::default() },
        "ganze Zeile ersetzt: nur noch die Größe (ohne Leerraum, Einheit klein), alles andere wieder aus der Vorlage"
    );
    assert_eq!(rows(&store).await, 1, "ersetzt, nicht angehängt");

    let blank = SheetSettings { font: Some("   ".into()), ..SheetSettings::default() };
    store.set_sheet_settings(work, &blank).await.unwrap();
    assert!(store.sheet_settings(work).await.unwrap().is_empty(), "nur Leerraum zählt als nicht gesetzt");
    assert_eq!(rows(&store).await, 0, "alles zurückgenommen löscht die Zeile");
}

#[tokio::test]
async fn invalid_values_are_refused_with_a_german_message_and_nothing_changes() {
    let (_dir, store, work) = fresh().await;
    store.set_sheet_settings(work, &full()).await.unwrap();
    let before = store.sheet_settings(work).await.unwrap();
    let long = "a".repeat(MAX_SHEET_FONT_CHARS + 1);
    let cases: Vec<(SheetSettings, &str)> = vec![
        (SheetSettings { font: Some("Serif; } body { color: red".into()), ..full() }, "Schrift"),
        (SheetSettings { font: Some("Probe</style><script>".into()), ..full() }, "Schrift"),
        (SheetSettings { font: Some("\"Offen, serif".into()), ..full() }, "Schrift"),
        (SheetSettings { font: Some("Zeile\u{1}Steuer".into()), ..full() }, "Schrift"),
        (SheetSettings { font: Some(long), ..full() }, "Schrift"),
        (SheetSettings { heading_font: Some("A{B}".into()), ..full() }, "Überschriften"),
        (SheetSettings { size: Some("4pt".into()), ..full() }, "Schriftgröße"),
        (SheetSettings { size: Some("11px".into()), ..full() }, "Schriftgröße"),
        (SheetSettings { size: Some("11".into()), ..full() }, "Schriftgröße"),
        (SheetSettings { width: Some("80cm".into()), ..full() }, "Satzbreite"),
        (SheetSettings { width: Some("calc(1cm)".into()), ..full() }, "Satzbreite"),
        (SheetSettings { fill: Some("red".into()), ..full() }, "Textfarbe"),
        (SheetSettings { heading_fill: Some("#abc".into()), ..full() }, "Überschriften"),
        (SheetSettings { background: Some("#12345g".into()), ..full() }, "Blattfarbe"),
    ];
    for (settings, field) in cases {
        let refused = store.set_sheet_settings(work, &settings).await;
        match refused {
            Err(StoreError::Refused(message)) => {
                assert!(message.starts_with("Satzbild:"), "Meldung beginnt mit „Satzbild:“: {message}");
                assert!(message.contains(field), "Meldung nennt das Feld {field}: {message}");
            }
            other => panic!("{settings:?} muss abgelehnt werden (Refused), war {other:?}"),
        }
        assert_eq!(store.sheet_settings(work).await.unwrap(), before, "nach der Ablehnung ist nichts geändert");
    }
    let edge = SheetSettings {
        font: Some("a".repeat(MAX_SHEET_FONT_CHARS)),
        size: Some("6pt".into()),
        width: Some("60cm".into()),
        ..SheetSettings::default()
    };
    store.set_sheet_settings(work, &edge).await.expect("Grenzwerte (200 Zeichen, 6 pt, 60 cm) sind erlaubt");
}

#[tokio::test]
async fn an_unknown_work_is_refused() {
    let (_dir, store, _) = fresh().await;
    assert!(
        matches!(store.sheet_settings(99_999).await, Err(StoreError::UnknownWork(99_999))),
        "Lesen für ein unbekanntes Werk: UnknownWork"
    );
    assert!(
        matches!(store.set_sheet_settings(99_999, &full()).await, Err(StoreError::UnknownWork(99_999))),
        "Setzen für ein unbekanntes Werk: UnknownWork"
    );
    assert_eq!(rows(&store).await, 0, "keine Zeile für ein unbekanntes Werk");
}

#[tokio::test]
async fn the_settings_never_reach_the_mirror_or_the_export_and_leave_with_the_work() {
    let (_dir, store, work) = fresh().await;
    let export_before = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let files_before = store.mirror_snapshot(work).await.unwrap();
    store.set_sheet_settings(work, &full()).await.unwrap();
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        export_before,
        "Export byte-gleich: Satzbild-Einstellungen ändern nie den Typst-Text"
    );
    let files_after = store.mirror_snapshot(work).await.unwrap();
    assert_eq!(files_after, files_before, "Spiegel-Schnappschuss unverändert: die Einstellungen stehen nur in SQLite");
    let title: String =
        sqlx::query_scalar("SELECT title FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    store.delete_work(work, &title).await.expect("Werk löschen");
    assert_eq!(rows(&store).await, 0, "mit dem Werk ist seine Zeile fort (ON DELETE CASCADE)");
}
