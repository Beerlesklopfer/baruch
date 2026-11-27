// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 21c (bilder) in the store on a real SQLite file in the temp folder: change of type when replacing
//! (`replace_asset_renaming`), path collisions in legacy data (`asset_path_conflicts`, `resolve_asset_path`),
//! requests with a change of type (`asset_request_with`), cleaning up decided requests
//! (`purge_decided_asset_requests`) and the counting for the MCP quota (`asset_mcp_usage`). No migration; all
//! images generated in the test.

use baruch_store::{
    AssetChangeKind, AssetEncoding, AssetOp, Decision, IdeaOrigin, NewAsset, Store, StoreError, WorkId,
};

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"8\"/></svg>\n";

/// Small "PNG": signature and invented bytes (the store recognises the type by the first bytes).
fn png(fill: u8, len: usize) -> Vec<u8> {
    let mut bytes = vec![fill; len.max(8)];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes
}

fn svg(n: usize) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect fill=\"#{n:06x}\"/></svg>\n")
}

async fn add(store: &Store, work: WorkId, file_name: &str, bytes: &[u8], path: Option<&str>, author: &str) {
    let new = NewAsset {
        file_name,
        name: None,
        bytes,
        as_base64: false,
        path,
        caption: "Unterschrift",
        alt: "Alt",
        note: "",
    };
    store.add_asset(work, &new, author).await.unwrap();
}

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    add(&store, work, "haus.svg", HOUSE.as_bytes(), Some("bilder/haus.svg\n/abb/haus.svg"), "Autor").await;
    (dir, store, work)
}

fn mcp(name: &str) -> IdeaOrigin {
    IdeaOrigin::Mcp { handle: format!("@mcp:{name}") }
}

fn replace(name: &str, content: &str, encoding: AssetEncoding) -> AssetOp {
    AssetOp::Replace {
        name: name.into(),
        mime: String::new(),
        encoding,
        content: content.into(),
        bytes: 0,
        caption: None,
        alt: None,
        expected_sha256: String::new(),
    }
}

#[tokio::test]
async fn another_format_needs_rename_and_then_keeps_paths_cover_and_open_requests() {
    let (_dir, store, work) = fresh().await;
    store.set_work_cover(work, Some("haus.svg")).await.unwrap();
    let asked = store
        .asset_request(
            work,
            &AssetOp::Delete { name: "haus.svg".into(), rev: None, expected_sha256: String::new() },
            &mcp("probe"),
            "",
        )
        .await
        .unwrap();
    let new = png(7, 40);
    let err = store.replace_asset(work, "haus.svg", "haus.png", &new, false, "", "Autor").await.unwrap_err();
    assert!(
        matches!(&err, StoreError::Asset(m) if m.contains("rename")),
        "ohne rename: Asset-Fehler mit Hinweis: {err}"
    );
    assert_eq!(store.asset_revisions(work, "haus.svg").await.unwrap().len(), 1, "nichts geändert");

    let change =
        store.replace_asset_renaming(work, "haus.svg", "haus.png", &new, false, "neu", "Autor", true).await.unwrap();
    assert_eq!(change.name, "haus.png", "Name mit der Vorgabe-Endung des neuen Formats");
    assert_eq!(change.kind, AssetChangeKind::Replaced, "eine neue Fassung");
    assert!(change.detail.contains("Name: haus.svg → haus.png"), "der Rumpf nennt die Umbenennung: {}", change.detail);
    let list = store.asset_list(work).await.unwrap();
    assert_eq!(list.len(), 1, "dasselbe Bild, nicht ein zweites");
    assert_eq!(list[0].name, "haus.png", "umbenannt");
    assert_eq!(list[0].mime, "image/png", "neuer Typ");
    assert_eq!(list[0].paths, ["/abb/haus.svg", "bilder/haus.svg"], "alle Pfade bleiben beim Bild");
    assert_eq!(list[0].revisions, 2, "beide Fassungen bleiben");
    let by_path = store.asset_by_path(work, "bilder/haus.svg").await.unwrap().unwrap();
    assert_eq!(by_path.bytes().unwrap(), new, "der alte Pfad liefert das neue Bild");
    assert_eq!(store.work_cover(work).await.unwrap().as_deref(), Some("haus.png"), "der Umschlag hängt an der id");
    let request = store.asset_request_get(work, asked).await.unwrap();
    assert_eq!(request.op.asset_name(), "haus.png", "ein offener Antrag zieht den Namen mit");
}

#[tokio::test]
async fn a_taken_name_gets_the_next_free_one_and_rename_with_the_same_format_keeps_the_name() {
    let (_dir, store, work) = fresh().await;
    add(&store, work, "haus.png", &png(1, 20), None, "Autor").await;
    let change =
        store.replace_asset_renaming(work, "haus.svg", "x.png", &png(2, 20), false, "", "Autor", true).await.unwrap();
    assert_eq!(change.name, "haus-2.png", "haus.png ist vergeben: der nächste freie Name");
    let same = store.replace_asset_renaming(work, "haus.png", "y.png", &png(3, 20), false, "", "Autor", true).await;
    assert_eq!(same.unwrap().name, "haus.png", "gleiches Format: rename ändert den Namen nicht");
}

#[tokio::test]
async fn path_collisions_of_the_old_kind_are_listed_and_resolved_for_the_chosen_picture() {
    let (_dir, store, work) = fresh().await;
    add(&store, work, "blau.png", &png(4, 16), None, "Autor").await;
    assert!(store.asset_path_conflicts(work).await.unwrap().is_empty(), "ohne Altbestand keine Kollision");
    // Rebuild legacy data (the store has refused that on creation since Round 7).
    sqlx::query(
        "INSERT INTO asset_path (work_id, path, asset_id) SELECT ?, './bilder/haus.svg', id FROM asset \
         WHERE work_id = ? AND name = 'blau.png'",
    )
    .bind(work)
    .bind(work)
    .execute(store.pool())
    .await
    .unwrap();
    let conflicts = store.asset_path_conflicts(work).await.unwrap();
    assert_eq!(conflicts.len(), 1, "eine Kollision: {conflicts:?}");
    assert_eq!(conflicts[0].normal, "bilder/haus.svg", "in Normalform");
    let owners: Vec<(&str, &str)> = conflicts[0].paths.iter().map(|p| (p.path.as_str(), p.asset.as_str())).collect();
    assert_eq!(owners, [("./bilder/haus.svg", "blau.png"), ("bilder/haus.svg", "haus.svg")], "Schreibweisen mit Bild");

    let err = store.resolve_asset_path(work, "bilder/haus.svg", "fremd.png", "Mensch").await.unwrap_err();
    assert!(matches!(err, StoreError::Asset(_)), "nur ein beteiligtes Bild steht zur Wahl: {err}");
    let change = store.resolve_asset_path(work, "/bilder/haus.svg", "haus.svg", "Mensch").await.unwrap();
    assert_eq!(change.kind, AssetChangeKind::Updated, "ein Spiegel-Commit am gewählten Bild");
    assert!(change.detail.contains("./bilder/haus.svg") && change.detail.contains("blau.png"), "{}", change.detail);
    assert!(store.asset_path_conflicts(work).await.unwrap().is_empty(), "danach keine Kollision");
    let list = store.asset_list(work).await.unwrap();
    let paths = |name: &str| list.iter().find(|a| a.name == name).unwrap().paths.clone();
    assert_eq!(paths("haus.svg"), ["./bilder/haus.svg", "/abb/haus.svg", "bilder/haus.svg"], "alle Schreibweisen");
    assert!(paths("blau.png").is_empty(), "das andere Bild verliert nur diesen Pfad");
    let again = store.resolve_asset_path(work, "bilder/haus.svg", "haus.svg", "Mensch").await;
    assert!(matches!(again, Err(StoreError::Refused(_))), "ohne Kollision nichts aufzulösen: {again:?}");
}

#[tokio::test]
async fn a_request_may_change_the_format_only_with_rename_and_says_so() {
    let (_dir, store, work) = fresh().await;
    let content = baruch_store::encode_base64(&png(5, 30));
    let op = replace("haus.svg", &content, AssetEncoding::Base64);
    let err = store.asset_request(work, &op, &mcp("probe"), "").await.unwrap_err();
    assert!(matches!(&err, StoreError::Asset(m) if m.contains("rename")), "ohne rename abgelehnt: {err}");
    let id = store.asset_request_with(work, &op, &mcp("probe"), "", true).await.unwrap();
    let request = store.asset_request_get(work, id).await.unwrap();
    assert!(request.rename, "der Antrag trägt den Typwechsel");
    assert!(request.summary.ends_with("mit neuer Endung"), "Zusammenfassung sagt es: {}", request.summary);
    let params: String = sqlx::query_scalar("SELECT params FROM asset_request WHERE id = ?")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(params.contains("\"rename\":true") && params.contains("\"bytes\":30"), "gespeichert: {params}");
}

#[tokio::test]
async fn decided_replace_requests_lose_their_content_after_the_given_days_only() {
    let (_dir, store, work) = fresh().await;
    let mut ids = Vec::new();
    for n in 0..3 {
        let op = replace("haus.svg", &svg(n + 1), AssetEncoding::Svg);
        ids.push(store.asset_request(work, &op, &mcp("probe"), "").await.unwrap());
    }
    for &id in &ids[..2] {
        store.asset_request_decide(work, id, Decision::Rejected, "Mensch", None).await.unwrap();
    }
    for (id, days) in [(ids[0], 31), (ids[1], 29)] {
        sqlx::query("UPDATE asset_request SET decided = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) WHERE id = ?")
            .bind(format!("-{days} days"))
            .bind(id)
            .execute(store.pool())
            .await
            .unwrap();
    }
    assert_eq!(store.purge_decided_asset_requests(work, 30).await.unwrap(), 1, "nur der eine alte, entschiedene");
    assert_eq!(store.purge_decided_asset_requests(work, 30).await.unwrap(), 0, "ein zweiter Lauf findet nichts mehr");
    let old = store.asset_request_get(work, ids[0]).await.unwrap();
    assert!(old.purged, "bereinigt");
    match &old.op {
        AssetOp::Replace { content, bytes, .. } => {
            assert!(content.is_empty(), "Inhalt entfernt");
            assert_eq!(*bytes, svg(1).len() as u64, "die Größe bleibt bekannt");
        }
        other => panic!("ein Antrag zum Ersetzen bleibt einer: {other:?}"),
    }
    assert_eq!(old.decision, Some(Decision::Rejected), "die Entscheidung bleibt");
    assert!(!store.asset_request_get(work, ids[1]).await.unwrap().purged, "29 Tage: bleibt");
    assert!(!store.asset_request_get(work, ids[2]).await.unwrap().purged, "offen: bleibt immer");
}

#[tokio::test]
async fn the_usage_of_a_client_counts_its_pictures_and_replace_requests_of_the_last_day() {
    let (_dir, store, work) = fresh().await;
    add(&store, work, "a.png", &png(1, 100), None, "@mcp:probe").await;
    add(&store, work, "b.png", &png(2, 50), None, "@mcp:probe").await;
    add(&store, work, "c.png", &png(3, 70), None, "@mcp:andere").await;
    let op = replace("haus.svg", &svg(9), AssetEncoding::Svg);
    let id = store.asset_request(work, &op, &mcp("probe"), "").await.unwrap();
    let usage = store.asset_mcp_usage(work, "@mcp:probe").await.unwrap();
    assert_eq!(usage.writes, 3, "zwei Bilder und ein Antrag zum Ersetzen: {usage:?}");
    assert_eq!(usage.bytes, 150 + svg(9).len() as u64, "Bytes entschlüsselt: {usage:?}");
    assert_eq!(usage.open_replace, 1, "ein offener Antrag");
    store.asset_request_decide(work, id, Decision::Rejected, "Mensch", None).await.unwrap();
    let usage = store.asset_mcp_usage(work, "@mcp:probe").await.unwrap();
    assert_eq!((usage.writes, usage.open_replace), (3, 0), "entschieden zählt weiter mit, ist aber nicht mehr offen");
    let other = store.asset_mcp_usage(work, "@mcp:andere").await.unwrap();
    assert_eq!((other.writes, other.bytes), (1, 70), "ein anderer Client zählt für sich");
    store.delete_asset(work, "a.png", "Mensch").await.unwrap();
    assert_eq!(
        store.asset_mcp_usage(work, "@mcp:probe").await.unwrap().writes,
        2,
        "gelöschte Bilder zählen nicht mehr"
    );
}
