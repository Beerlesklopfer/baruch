// SPDX-License-Identifier: AGPL-3.0-or-later
//! Requests about images (migration 0018, contract Round 11) on a real SQLite file in the temp folder: checking at
//! the request, expected state, no duplicates, decision exactly once, deleting a work deletes its requests too. All
//! images are generated in the test.

use baruch_store::{AssetEncoding, AssetOp, Decision, IdeaOrigin, NewAsset, Store, StoreError, WorkId};

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"8\"/></svg>\n";
const HOUSE_RED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect fill=\"red\" width=\"10\" height=\"8\"/></svg>\n";

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    let new = NewAsset {
        file_name: "haus.svg",
        name: None,
        bytes: HOUSE.as_bytes(),
        as_base64: false,
        path: None,
        caption: "Das Haus",
        alt: "",
        note: "",
    };
    store.add_asset(work, &new, "Autor").await.unwrap();
    (dir, store, work)
}

fn mcp() -> IdeaOrigin {
    IdeaOrigin::Mcp { handle: "@mcp:probe".into() }
}

fn replace(content: &str) -> AssetOp {
    AssetOp::Replace {
        name: "haus.svg".into(),
        mime: String::new(),
        encoding: AssetEncoding::Svg,
        content: content.into(),
        bytes: 0,
        caption: None,
        alt: Some("Rotes Haus".into()),
        expected_sha256: String::new(),
    }
}

#[tokio::test]
async fn a_request_changes_nothing_and_remembers_the_current_state() {
    let (_dir, store, work) = fresh().await;
    let before = store.asset_revisions(work, "haus.svg").await.unwrap();
    let id = store.asset_request(work, &replace(HOUSE_RED), &mcp(), " röter ").await.unwrap();
    assert_eq!(store.asset_revisions(work, "haus.svg").await.unwrap(), before, "ein Antrag ändert am Bild nichts");
    let request = store.asset_request_get(work, id).await.unwrap();
    assert!(request.is_open(), "neu angelegt ist der Antrag offen");
    assert_eq!(request.note, "röter", "die Begründung wird getrimmt gespeichert");
    assert_eq!(request.origin, mcp(), "Herkunft bleibt erhalten");
    match &request.op {
        AssetOp::Replace { mime, bytes, expected_sha256, content, .. } => {
            assert_eq!(mime, "image/svg+xml", "der Store erkennt den Typ");
            assert_eq!(*bytes, HOUSE_RED.len() as u64, "Größe entschlüsselt");
            assert_eq!(expected_sha256, &before[0].sha256, "erwarteter Stand = SHA-256 der aktuellen Fassung");
            assert_eq!(content, HOUSE_RED, "der Inhalt ist für die Ausführung gespeichert");
        }
        other => panic!("Antrag zum Ersetzen erwartet, nicht {other:?}"),
    }
    assert!(request.summary.starts_with("„haus.svg“ durch eine neue Fassung ersetzen (SVG"), "{}", request.summary);
    assert_eq!(store.open_asset_requests(work).await.unwrap(), 1, "ein offener Antrag");
    let err = store.asset_request(work, &replace(HOUSE_RED), &mcp(), "röter").await.unwrap_err();
    assert!(matches!(err, StoreError::Refused(_)), "derselbe offene Antrag kein zweites Mal: {err:?}");
}

#[tokio::test]
async fn what_could_not_be_done_now_is_no_request() {
    let (_dir, store, work) = fresh().await;
    let unchanged = store.asset_request(work, &replace(HOUSE), &mcp(), "").await.unwrap_err();
    assert!(matches!(unchanged, StoreError::Refused(_)), "gleicher Inhalt → kein Antrag: {unchanged:?}");
    let mut wrong = replace(HOUSE_RED);
    if let AssetOp::Replace { name, .. } = &mut wrong {
        *name = "fehlt.svg".into();
    }
    let unknown = store.asset_request(work, &wrong, &mcp(), "").await.unwrap_err();
    assert!(matches!(unknown, StoreError::UnknownAsset { .. }), "unbekanntes Bild: {unknown:?}");
    let png = AssetOp::Replace {
        name: "haus.svg".into(),
        mime: String::new(),
        encoding: AssetEncoding::Base64,
        content: baruch_store::encode_base64(b"\x89PNG\r\n\x1a\nkaputt"),
        bytes: 0,
        caption: None,
        alt: None,
        expected_sha256: String::new(),
    };
    let other = store.asset_request(work, &png, &mcp(), "").await.unwrap_err();
    assert!(matches!(other, StoreError::Asset(_)), "kein Bild bzw. anderer Typ als das Bild: {other:?}");
    let big =
        "<svg xmlns=\"http://www.w3.org/2000/svg\">".to_owned() + &" ".repeat(baruch_store::MAX_ASSET_BYTES) + "</svg>";
    let large = store.asset_request(work, &replace(&big), &mcp(), "").await.unwrap_err();
    assert!(large.to_string().contains("8 MiB"), "über 8 MiB: {large}");
    let mut stale = replace(HOUSE_RED);
    if let AssetOp::Replace { expected_sha256, .. } = &mut stale {
        *expected_sha256 = "0".repeat(64);
    }
    let old = store.asset_request(work, &stale, &mcp(), "").await.unwrap_err();
    assert!(old.to_string().contains("expected_sha256"), "falscher erwarteter Stand gleich beim Antrag: {old}");
    let current = AssetOp::Delete { name: "haus.svg".into(), rev: Some(1), expected_sha256: String::new() };
    let refused = store.asset_request(work, &current, &mcp(), "").await.unwrap_err();
    assert!(matches!(refused, StoreError::Refused(_)), "die aktuelle Fassung allein löschen geht nicht: {refused:?}");
    let human = IdeaOrigin::Human { name: "Autor".into() };
    let by_human = store.asset_request(work, &replace(HOUSE_RED), &human, "").await.unwrap_err();
    assert!(matches!(by_human, StoreError::Refused(_)), "der Mensch ändert direkt: {by_human:?}");
    assert_eq!(store.open_asset_requests(work).await.unwrap(), 0, "keiner der Versuche ist ein Antrag");
}

#[tokio::test]
async fn a_decision_is_recorded_once_and_lists_follow_it() {
    let (_dir, store, work) = fresh().await;
    let cover = AssetOp::SetCover { name: Some("haus.svg".into()), expected_cover: None };
    let first = store.asset_request(work, &cover, &IdeaOrigin::Persona { handle: "@lektor".into() }, "").await.unwrap();
    let all = AssetOp::Delete { name: "haus.svg".into(), rev: None, expected_sha256: String::new() };
    let second = store.asset_request(work, &all, &mcp(), "weg damit").await.unwrap();
    assert_eq!(store.asset_requests(work, true).await.unwrap().len(), 2, "zwei offene");

    let rejected = store.asset_request_decide(work, first, Decision::Rejected, "Autor", None).await.unwrap();
    assert_eq!(rejected.decision, Some(Decision::Rejected), "abgelehnt");
    assert_eq!(rejected.decided_by.as_deref(), Some("Autor"), "der Mensch steht als Entscheider da");
    assert!(rejected.decided.is_some(), "mit Zeitpunkt");
    let again = store.asset_request_decide(work, first, Decision::Confirmed, "Autor", None).await.unwrap_err();
    assert!(matches!(again, StoreError::Refused(_)), "über einen Antrag wird genau einmal entschieden: {again:?}");

    let stale =
        store.asset_request_decide(work, second, Decision::Stale, "Autor", Some("Bild geändert")).await.unwrap();
    assert_eq!(stale.message.as_deref(), Some("Bild geändert"), "veraltet trägt den Grund");
    assert!(store.asset_requests(work, true).await.unwrap().is_empty(), "keine offenen mehr");
    let all = store.asset_requests(work, false).await.unwrap();
    assert_eq!(all.iter().map(|r| r.id).collect::<Vec<_>>(), [first, second], "alle nach id");
    let unknown = store.asset_request_decide(work, 999, Decision::Rejected, "Autor", None).await.unwrap_err();
    assert!(matches!(unknown, StoreError::UnknownRequest { .. }), "unbekannter Antrag: {unknown:?}");
}

#[tokio::test]
async fn removing_the_cover_needs_no_image_and_deleting_the_work_deletes_its_requests() {
    let (_dir, store, work) = fresh().await;
    let off = AssetOp::SetCover { name: None, expected_cover: Some("haus.svg".into()) };
    let id = store.asset_request(work, &off, &mcp(), "").await.unwrap();
    let request = store.asset_request_get(work, id).await.unwrap();
    assert_eq!(request.op, off, "Umschlag entfernen: ohne Bild, mit dem Umschlag beim Antrag");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM asset_request").fetch_one(store.pool()).await.unwrap();
    assert_eq!(rows, 1, "eine Zeile");
    let title: String =
        sqlx::query_scalar("SELECT title FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    store.delete_work(work, &title).await.expect("Werk mit Bildantrag lässt sich löschen");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM asset_request").fetch_one(store.pool()).await.unwrap();
    assert_eq!(rows, 0, "ON DELETE CASCADE: die Anträge gehen mit dem Werk");
}
