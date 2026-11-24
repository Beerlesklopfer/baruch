// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschlag-Anordnung im Store (Migration 0045, Vertrag Runde 27, Aufgabe umschlag-daten): Lesen, Setzen je Art,
//! Prüfen nach den Grenzen, Zurücksetzen mit Grabsteinen und Titelbereich in einer Transaktion, uuid je Block,
//! Anschriften in den Buchangaben und Anträge mit Fingerabdruck. Echte SQLite-Datei im Temp-Ordner; alle Texte frei
//! erfunden; jede Zusicherung mit Begründung.

use baruch_store::{
    AssetOp, BookEdit, CoverBlock, CoverBlockKind, CoverFace, CoverLayout, IdeaOrigin, Store, StoreError, TitleArea,
    WorkId,
};
use baruch_typst::cover_layout::{Backdrop, CoverAlign, FontStyle, SpineDirection};

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

fn block(kind: CoverBlockKind) -> CoverBlock {
    CoverBlock {
        kind,
        x_mm: 12.0,
        y_mm: 30.0,
        width_mm: 120.0,
        align: CoverAlign::Center,
        size_pt: 26.0,
        leading_em: 0.65,
        color: "light".into(),
        font: None,
        style: FontStyle::Bold,
        backdrop: Backdrop::Shadow,
        visible: true,
        direction: (kind.face() == CoverFace::Spine).then_some(SpineDirection::Down),
        text: kind.own_text().then(|| "Ein erfundener Freitext".to_owned()),
    }
}

async fn rows(store: &Store, work: WorkId) -> Vec<(String, String)> {
    sqlx::query_as("SELECT kind, uuid FROM cover_block WHERE work_id = ? ORDER BY kind")
        .bind(work)
        .fetch_all(store.pool())
        .await
        .unwrap()
}

async fn tombstones(store: &Store) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as("SELECT uuid, kind, work_uuid FROM uuid_tombstone WHERE kind = 'cover_block' ORDER BY uuid")
        .fetch_all(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_work_without_rows_has_no_layout_and_unknown_works_are_refused() {
    let (_dir, store, work) = fresh().await;
    assert!(store.cover_layout(work).await.unwrap().is_empty(), "ohne Zeile keine eigene Anordnung (Vertrag 1.4)");
    assert!(matches!(store.cover_layout(9999).await, Err(StoreError::UnknownWork(9999))), "unbekanntes Werk");
    assert!(
        matches!(
            store.set_cover_blocks(9999, &[block(CoverBlockKind::FrontTitle)]).await,
            Err(StoreError::UnknownWork(_))
        ),
        "Setzen in einem unbekannten Werk"
    );
}

#[tokio::test]
async fn setting_replaces_each_named_kind_wholly_and_keeps_the_others_and_their_uuid() {
    let (_dir, store, work) = fresh().await;
    let title = block(CoverBlockKind::FrontTitle);
    let spine = block(CoverBlockKind::SpineTitle);
    let change = store.set_cover_blocks(work, &[spine.clone(), title.clone()]).await.unwrap();
    assert_eq!(change.changed, [CoverBlockKind::FrontTitle, CoverBlockKind::SpineTitle], "Arten in ALL-Reihenfolge");
    assert!(change.before.is_empty(), "vorher keine Anordnung");
    let layout = store.cover_layout(work).await.unwrap();
    assert_eq!(layout.blocks, vec![title.clone(), spine.clone()], "gespeichert in Normalform, Reihenfolge der Arten");
    let first = rows(&store, work).await;
    assert!(first.iter().all(|(_, u)| u.len() == 36), "Fülltrigger vergibt je Block eine uuid: {first:?}");

    // Ganze Art ersetzt: neue Werte für den Titel, Rücken ungenannt → bleibt.
    let mut moved = title.clone();
    moved.x_mm = 15.0;
    moved.font = Some("Libertinus Serif".into());
    moved.style = FontStyle::Normal;
    let change = store.set_cover_blocks(work, &[moved.clone()]).await.unwrap();
    assert_eq!(change.changed, [CoverBlockKind::FrontTitle], "nur der genannte Block ist geändert");
    assert_eq!(change.before.get(CoverBlockKind::FrontTitle), Some(&title), "vorher für den Commit-Rumpf");
    assert_eq!(change.after.get(CoverBlockKind::FrontTitle), Some(&moved), "nachher für den Commit-Rumpf");
    assert_eq!(store.cover_layout(work).await.unwrap().get(CoverBlockKind::SpineTitle), Some(&spine), "Rücken bleibt");
    assert_eq!(rows(&store, work).await, first, "geänderter Block behält seine uuid (kein Löschen, kein Grabstein)");
    assert!(tombstones(&store).await.is_empty(), "Ändern hinterlässt keinen Grabstein");

    // Gleiche Werte noch einmal: keine Änderung.
    let again = store.set_cover_blocks(work, &[moved]).await.unwrap();
    assert!(again.changed.is_empty(), "unverändert → changed leer (kein Commit im Server)");
}

#[tokio::test]
async fn invalid_blocks_are_refused_with_kind_and_field_and_change_nothing() {
    let (_dir, store, work) = fresh().await;
    store.set_cover_blocks(work, &[block(CoverBlockKind::BackBlurb)]).await.unwrap();
    let before = store.cover_layout(work).await.unwrap();
    let cases: Vec<(CoverBlock, &str)> = vec![
        (CoverBlock { x_mm: 1000.1, ..block(CoverBlockKind::FrontTitle) }, "x_mm"),
        (CoverBlock { y_mm: -50.1, ..block(CoverBlockKind::FrontTitle) }, "y_mm"),
        (CoverBlock { width_mm: 0.9, ..block(CoverBlockKind::FrontTitle) }, "width_mm"),
        (CoverBlock { size_pt: 3.9, ..block(CoverBlockKind::FrontTitle) }, "size_pt"),
        (CoverBlock { leading_em: 3.1, ..block(CoverBlockKind::FrontTitle) }, "leading_em"),
        (CoverBlock { color: "rot".into(), ..block(CoverBlockKind::FrontTitle) }, "color"),
        (CoverBlock { direction: Some(SpineDirection::Up), ..block(CoverBlockKind::FrontTitle) }, "direction"),
        (CoverBlock { text: Some("eigener Titel".into()), ..block(CoverBlockKind::FrontTitle) }, "text"),
        (CoverBlock { text: Some("x".repeat(1001)), ..block(CoverBlockKind::BackFree) }, "text"),
        (CoverBlock { font: Some("Serif; } x {".into()), ..block(CoverBlockKind::FrontTitle) }, "font"),
        (CoverBlock { font: Some("A<b>".into()), ..block(CoverBlockKind::FrontTitle) }, "font"),
        (CoverBlock { font: Some("A\\B".into()), ..block(CoverBlockKind::FrontTitle) }, "font"),
        (CoverBlock { font: Some("A\u{1b}B".into()), ..block(CoverBlockKind::FrontTitle) }, "font"),
        (CoverBlock { font: Some("a".repeat(201)), ..block(CoverBlockKind::FrontTitle) }, "font"),
    ];
    for (bad, field) in cases {
        let kind = bad.kind;
        // Der gültige zweite Block zeigt: die ganze Liste wird abgelehnt (atomar), nicht nur der ungültige Block.
        let result = store.set_cover_blocks(work, &[block(CoverBlockKind::BackBio), bad]).await;
        let message = match result {
            Err(StoreError::CoverBlock(message)) => message,
            other => panic!("{kind:?}/{field}: CoverBlock-Fehler (RPC -32602, Runde 23a) erwartet, nicht {other:?}"),
        };
        assert!(message.contains(kind.as_str()), "Meldung nennt die Art ({kind:?}): {message}");
        assert!(message.contains(&format!("Feld {field}")), "Meldung nennt das Feld {field}: {message}");
        assert_eq!(store.cover_layout(work).await.unwrap(), before, "{kind:?}/{field}: nichts geändert");
    }
    let twice =
        store.set_cover_blocks(work, &[block(CoverBlockKind::FrontTitle), block(CoverBlockKind::FrontTitle)]).await;
    assert!(
        matches!(twice, Err(StoreError::CoverBlock(ref m)) if m.contains("zweimal")),
        "dieselbe Art zweimal ist ein ungültiger Block (eigene Fehlerart seit Runde 23a): {twice:?}"
    );
    // Grenzwerte selbst sind erlaubt (BETWEEN in den CHECKs ist einschließlich).
    let edge = CoverBlock {
        x_mm: -50.0,
        y_mm: 1000.0,
        width_mm: 1.0,
        size_pt: 200.0,
        leading_em: 0.0,
        ..block(CoverBlockKind::FrontTitle)
    };
    store.set_cover_blocks(work, std::slice::from_ref(&edge)).await.unwrap();
    assert_eq!(store.cover_layout(work).await.unwrap().get(CoverBlockKind::FrontTitle), Some(&edge), "Grenzwerte");
}

#[tokio::test]
async fn resetting_all_deletes_every_row_and_leaves_tombstones() {
    let (_dir, store, work) = fresh().await;
    let all: Vec<CoverBlock> = CoverBlockKind::ALL.into_iter().map(block).collect();
    store.set_cover_blocks(work, &all).await.unwrap();
    let uuids: Vec<String> = rows(&store, work).await.into_iter().map(|(_, u)| u).collect();
    assert_eq!(uuids.len(), 15, "Vorbedingung: alle 15 Arten gespeichert");
    let change = store.reset_cover_blocks(work, &[], None).await.unwrap();
    assert_eq!(change.changed.len(), 15, "alle Arten gelöscht");
    assert!(change.after.is_empty() && store.cover_layout(work).await.unwrap().is_empty(), "keine Zeile mehr");
    let work_uuid = store.work_uuid(work).await.unwrap();
    let stones = tombstones(&store).await;
    assert_eq!(stones.len(), 15, "je gelöschter Zeile ein Grabstein 'cover_block'");
    for (uuid, kind, owner) in &stones {
        assert!(uuids.contains(uuid), "Grabstein trägt die uuid des Blocks: {uuid}");
        assert_eq!((kind.as_str(), owner.as_deref()), ("cover_block", Some(work_uuid.as_str())), "Art und Werk-uuid");
    }
}

#[tokio::test]
async fn a_quick_choice_sets_the_title_area_and_deletes_only_the_front_in_one_transaction() {
    let (_dir, store, work) = fresh().await;
    let blocks =
        [block(CoverBlockKind::FrontTitle), block(CoverBlockKind::FrontAuthor), block(CoverBlockKind::SpineTitle)];
    store.set_cover_blocks(work, &blocks).await.unwrap();
    let change = store.reset_cover_blocks(work, &[CoverFace::Front], Some(TitleArea::Bottom)).await.unwrap();
    assert_eq!(change.changed, [CoverBlockKind::FrontTitle, CoverBlockKind::FrontAuthor], "nur die Vorderseite");
    let layout = store.cover_layout(work).await.unwrap();
    assert_eq!(layout.blocks, vec![blocks[2].clone()], "Rücken bleibt stehen");
    assert_eq!(store.book_meta(work).await.unwrap().title_area, TitleArea::Bottom, "Titelbereich gesetzt");
    // Ohne Zeilen der Fläche und mit gleichem Bereich: nichts zu tun.
    let none = store.reset_cover_blocks(work, &[CoverFace::Front], Some(TitleArea::Bottom)).await.unwrap();
    assert!(none.changed.is_empty(), "zweite Schnellwahl löscht nichts");
}

#[tokio::test]
async fn a_failing_reset_changes_neither_rows_nor_title_area() {
    // Atomar: Zeigt die Transaktion einen Fehler (hier: unbekanntes Werk), bleibt alles. Das unbekannte Werk wird
    // geprüft, bevor gelöscht wird; das bekannte bleibt unberührt.
    let (_dir, store, work) = fresh().await;
    store.set_cover_blocks(work, &[block(CoverBlockKind::FrontTitle)]).await.unwrap();
    assert!(matches!(
        store.reset_cover_blocks(9999, &[], Some(TitleArea::Middle)).await,
        Err(StoreError::UnknownWork(9999))
    ));
    assert_eq!(store.cover_layout(work).await.unwrap().blocks.len(), 1, "anderes Werk unberührt");
}

#[tokio::test]
async fn deleting_the_work_leaves_tombstones_with_the_work_uuid() {
    let (_dir, store, work) = fresh().await;
    store.set_cover_blocks(work, &[block(CoverBlockKind::BackMotto)]).await.unwrap();
    let work_uuid = store.work_uuid(work).await.unwrap();
    let title = store.works().await.unwrap().into_iter().find(|w| w.0 == work).unwrap().2;
    store.delete_work(work, &title).await.unwrap();
    let stones = tombstones(&store).await;
    assert_eq!(stones.len(), 1, "der Block hinterlässt einen Grabstein");
    assert_eq!(stones[0].2.as_deref(), Some(work_uuid.as_str()), "Trigger vor dem Werk: Werk-uuid bekannt (0045)");
}

#[tokio::test]
async fn addresses_are_multiline_up_to_500_characters() {
    let (_dir, store, work) = fresh().await;
    let edit = BookEdit {
        publisher_address: Some("Probeverlag\r\nAm Brunnen 1\n12345 Musterstadt  ".into()),
        printer_address: Some("Probedruck\nDruckgasse 2".into()),
        ..BookEdit::default()
    };
    let change = store.set_book_meta(work, &edit).await.unwrap();
    assert_eq!(change.changed, ["publisher_address", "printer_address"], "beide Anschriften geändert");
    let meta = store.book_meta(work).await.unwrap();
    assert_eq!(
        meta.publisher_address, "Probeverlag\nAm Brunnen 1\n12345 Musterstadt",
        "mehrzeilig, CRLF → LF, getrimmt"
    );
    assert_eq!(meta.printer_address, "Probedruck\nDruckgasse 2", "Anschrift der Druckerei gespeichert");
    let long = BookEdit { printer_address: Some("ä".repeat(501)), ..BookEdit::default() };
    let refused = store.set_book_meta(work, &long).await;
    assert!(
        matches!(refused, Err(StoreError::Refused(m)) if m == "Anschrift der Druckerei: höchstens 500 Zeichen."),
        "501 Zeichen (nicht Bytes) → deutsche Meldung: {refused:?}"
    );
    assert_eq!(store.book_meta(work).await.unwrap().printer_address, "Probedruck\nDruckgasse 2", "nichts geändert");
    let exact = BookEdit { printer_address: Some("ä".repeat(500)), ..BookEdit::default() };
    store.set_book_meta(work, &exact).await.unwrap();
    assert_eq!(store.book_meta(work).await.unwrap().printer_address.chars().count(), 500, "genau 500 Zeichen gehen");
    // Andere Felder ändern lässt die Anschriften stehen (ganze Zeile wird geschrieben).
    store.set_book_meta(work, &BookEdit { year: Some("2026".into()), ..BookEdit::default() }).await.unwrap();
    assert_eq!(store.book_meta(work).await.unwrap().publisher_address.lines().count(), 3, "Anschrift bleibt");
}

fn mcp() -> IdeaOrigin {
    IdeaOrigin::Mcp { handle: "@mcp:probe".into() }
}

#[tokio::test]
async fn a_layout_request_remembers_the_fingerprint_changes_nothing_and_is_not_doubled() {
    let (_dir, store, work) = fresh().await;
    store.set_cover_blocks(work, &[block(CoverBlockKind::FrontTitle)]).await.unwrap();
    let before = store.cover_layout(work).await.unwrap();
    let mut wanted = block(CoverBlockKind::FrontTitle);
    wanted.y_mm = 80.0;
    wanted.color = "#ABCDEF".into();
    let op = AssetOp::CoverLayout { blocks: vec![wanted.clone()], expected_layout: String::new() };
    let id = store.asset_request(work, &op, &mcp(), "Titel tiefer").await.unwrap();
    assert_eq!(store.cover_layout(work).await.unwrap(), before, "der Antrag ändert die Anordnung nicht");
    let request = store.asset_request_get(work, id).await.unwrap();
    let AssetOp::CoverLayout { blocks, expected_layout } = &request.op else { panic!("op cover_layout") };
    assert_eq!(expected_layout, &Store::cover_layout_fingerprint(&before), "Fingerabdruck des Stands beim Antrag");
    assert_eq!(blocks[0].color, "#abcdef", "gespeichert in Normalform (so trifft der Doppel-Vergleich)");
    assert_eq!(request.summary, "Umschlag-Anordnung ändern (1 Block)", "Zusammenfassung deutsch");
    assert!(request.is_open(), "offen");
    let twin = store.asset_request(work, &op, &mcp(), "noch einmal").await;
    assert!(matches!(twin, Err(StoreError::Refused(m)) if m.contains("schon")), "kein doppelter offener Antrag");
    // Ein Antrag, der nichts ändern würde, und ein ungültiger Block ergeben keinen Antrag.
    let same = AssetOp::CoverLayout { blocks: before.blocks.clone(), expected_layout: String::new() };
    assert!(matches!(store.asset_request(work, &same, &mcp(), "").await, Err(StoreError::Refused(_))), "unverändert");
    let bad = AssetOp::CoverLayout {
        blocks: vec![CoverBlock { size_pt: 500.0, ..block(CoverBlockKind::FrontTitle) }],
        expected_layout: String::new(),
    };
    assert!(
        matches!(store.asset_request(work, &bad, &mcp(), "").await, Err(StoreError::CoverBlock(_))),
        "ein ungültiger Block ergibt keinen Antrag, sondern den Block-Fehler (Runde 23a)"
    );
    let stale = AssetOp::CoverLayout { blocks: vec![wanted], expected_layout: "0".repeat(64) };
    assert!(
        matches!(store.asset_request(work, &stale, &mcp(), "").await, Err(StoreError::Asset(m)) if m.contains("expected_layout")),
        "ein falscher Fingerabdruck beim Antrag wird abgelehnt"
    );
    // Der Mensch ändert direkt: kein Antrag mit Herkunft Mensch.
    let human = IdeaOrigin::Human { name: "Autor".into() };
    assert!(matches!(store.asset_request(work, &op, &human, "").await, Err(StoreError::Refused(_))), "Mensch direkt");
    let listed = store.asset_requests(work, true).await.unwrap();
    assert_eq!(listed.len(), 1, "genau ein offener Antrag");
    assert_eq!(serde_json::to_value(&listed[0]).unwrap()["op"], "cover_layout", "op im JSON");
}

#[test]
fn the_fingerprint_of_an_empty_layout_is_stable() {
    // Leere Anordnung → fester Wert (SHA-256 über „[]“): ein Antrag auf ein Werk ohne Anordnung trägt ihn.
    assert_eq!(
        Store::cover_layout_fingerprint(&CoverLayout::default()),
        "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        "SHA-256 von []"
    );
}
