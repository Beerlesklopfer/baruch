// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (side branches, ownership fassungen): remaining revisions, open proposals and variants as refs
//! `refs/revisions/<key>/<n>` in the mirror – created after every revision commit, updated on rejecting and at start
//! (`sync_work`), removed on deleting the revision. Reading is done through `baruch_gitmirror` (without a git program)
//! and, where present, cross-checked with `git` in the test folder. Freely invented work, no model, no network.

use crate::common;

use std::collections::BTreeMap;

use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Die Schleuse am Kanal\", author: \"Ole Brandt\")\n\n\
= Hochwasser\n\nDas Wasser stand bis an die Kante der Schleuse, und der Wärter zählte die Stufen am Pegel.\n\n\
= Ebbe\n\nAm Morgen lag der Kanal still, und die Boote warteten im Schlick.\n";

const PREFIX: &str = "refs/revisions/";

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn setup(server: &common::Server) -> (i64, i64, String) {
    let report = server.import(BOOK, "schleuse.typ").await;
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 0 })).await;
    (report.work, result(&node)["key"].as_i64().unwrap(), report.slug)
}

async fn side_refs(server: &common::Server, work: i64) -> BTreeMap<String, String> {
    server.state.mirror(work).await.unwrap().side_refs(PREFIX).unwrap()
}

async fn body_of(server: &common::Server, work: i64, rev: u32) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes[0].revision(rev).unwrap().body.clone()
}

#[tokio::test]
async fn other_versions_become_side_refs_with_the_section_in_that_version_and_head_as_parent() {
    let server = common::open().await;
    let (work, key, slug) = setup(&server).await;
    assert!(side_refs(&server, work).await.is_empty(), "frisch importiert: nur aktuelle Fassungen, keine Seitenzweige");
    let first = body_of(&server, work, 1).await;
    let second = first.replacen("Schleuse", "alten Schleuse", 1);
    let saved = server
        .rpc("outline.save", json!({ "work": work, "key": key, "body": second, "based_on": 1, "mode": "new" }))
        .await;
    assert_eq!(result(&saved)["rev"].as_u64(), Some(2), "neue Fassung 2");

    let refs = side_refs(&server, work).await;
    let name = format!("{PREFIX}{key}/1");
    assert_eq!(refs.keys().collect::<Vec<_>>(), [&name], "ersetzte Fassung 1 als Seitenzweig");
    let mirror = server.state.mirror(work).await.unwrap();
    let head = mirror.head().unwrap().unwrap();
    assert_eq!(mirror.parents(&name).unwrap(), Some(vec![head.clone()]), "Elternteil ist HEAD (der Speicher-Commit)");
    let path = std::fs::read_dir(server.mirror_dir_of(&slug).join("abschnitte"))
        .unwrap()
        .map(|e| format!("abschnitte/{}", e.unwrap().file_name().to_string_lossy()))
        .find(|p| p.starts_with("abschnitte/000-"))
        .expect("Abschnittsdatei 000");
    let side = mirror.read_file(&name, &path).unwrap().expect("Abschnitt im Seitenzweig");
    assert!(side.contains("bis an die Kante der Schleuse") && !side.contains("alten Schleuse"), "Fassung 1: {side}");
    assert!(side.contains(&format!("<k{key}>")), "Sprungmarke wie im Spiegel: {side}");
    let current = mirror.read_file("HEAD", &path).unwrap().unwrap();
    assert!(current.contains("alten Schleuse"), "HEAD trägt die aktuelle Fassung 2");
    assert_eq!(
        mirror.read_file(&name, "main.typ").unwrap(),
        mirror.read_file("HEAD", "main.typ").unwrap(),
        "übrige Dateien wie im Schnappschuss"
    );
    let dir = server.mirror_dir_of(&slug);
    if let Some(listed) = common::git(&dir, &["for-each-ref", "--format=%(refname)", PREFIX]) {
        assert_eq!(listed.trim(), name, "git sieht dieselbe Referenz");
        let message = common::git(&dir, &["log", "-1", "--format=%B", &name]).unwrap();
        assert!(message.contains("Fassung 1 (ersetzt) als Seitenzweig"), "Betreff: {message}");
        assert!(
            message.contains("Baruch-Side: superseded") && message.contains(&format!("Baruch-Key: {key}")),
            "Trailer Seite und Schlüssel: {message}"
        );
        let status = common::git(&dir, &["status", "--porcelain"]).unwrap();
        assert_eq!(status, "", "Arbeitsordner sauber (Seitenzweige berühren ihn nicht)");
    }

    // A further save commit without a change to revision 1: the side branch stays the same commit.
    let before = side_refs(&server, work).await;
    let amended = second.replacen("Pegel", "alten Pegel", 1);
    server
        .rpc("outline.save", json!({ "work": work, "key": key, "body": amended, "based_on": 2, "mode": "auto" }))
        .await;
    assert_eq!(side_refs(&server, work).await, before, "unveränderte Fassung: Seitenzweig nicht neu geschrieben");
}

#[tokio::test]
async fn proposals_and_rejections_are_tracked_and_deleting_a_version_removes_its_ref() {
    let server = common::open().await;
    let (work, key, _slug) = setup(&server).await;
    let first = body_of(&server, work, 1).await;
    let proposal = server
        .state
        .store
        .add_proposal(work, 0, &first.replacen("Wärter", "müde Wärter", 1), "@lektor", "mock", "kürzer")
        .await
        .unwrap();
    // A proposal is no commit; the next revision commit (here: new revision in section 1) catches it up.
    let other = server.rpc("node.get", json!({ "work": work, "node": 1 })).await;
    let other_key = result(&other)["key"].as_i64().unwrap();
    let body = result(&other)["revisions"][0]["body"].as_str().unwrap().replacen("still", "ganz still", 1);
    server.rpc("outline.save", json!({ "work": work, "key": other_key, "body": body, "based_on": 1 })).await;
    let name = format!("{PREFIX}{key}/{proposal}");
    let refs = side_refs(&server, work).await;
    assert!(refs.contains_key(&name), "offener Vorschlag als Seitenzweig: {refs:?}");
    assert!(refs.contains_key(&format!("{PREFIX}{other_key}/1")), "ersetzte Fassung des anderen Abschnitts: {refs:?}");
    let mirror = server.state.mirror(work).await.unwrap();
    let dir = mirror.dir().to_path_buf();
    if let Some(message) = common::git(&dir, &["log", "-1", "--format=%B", &name]) {
        assert!(message.contains("Baruch-Side: proposed") && message.contains("Persona: @lektor"), "{message}");
    }

    // Rejecting: no commit, but the side branch carries the status afterwards.
    let before = refs[&name].clone();
    result(&server.rpc("revision.reject", json!({ "work": work, "key": key, "rev": proposal })).await);
    let refs = side_refs(&server, work).await;
    assert_ne!(refs[&name], before, "Status geändert: Seitenzweig neu geschrieben");
    if let Some(message) = common::git(&dir, &["log", "-1", "--format=%B", &name]) {
        assert!(message.contains("Baruch-Side: rejected"), "verworfen: {message}");
    }

    // Deleting the revision removes the side branch.
    let deleted =
        server.rpc("revision.delete", json!({ "work": work, "key": key, "rev": proposal, "confirm": true })).await;
    result(&deleted);
    assert!(!side_refs(&server, work).await.contains_key(&name), "gelöschte Fassung: kein Seitenzweig mehr");
}

#[tokio::test]
async fn the_start_brings_missing_side_refs_back() {
    let server = common::open().await;
    let (work, key, _slug) = setup(&server).await;
    let first = body_of(&server, work, 1).await;
    server
        .rpc(
            "outline.save",
            json!({ "work": work, "key": key, "body": first.clone() + "\nNachtrag.\n", "based_on": 1 }),
        )
        .await;
    let name = format!("{PREFIX}{key}/1");
    let mirror = server.state.mirror(work).await.unwrap();
    assert!(mirror.side_refs(PREFIX).unwrap().contains_key(&name), "Seitenzweig nach dem Speichern");
    // Side branches gone from outside (e.g. an older mirror without them): the start catches them up (sync_work).
    mirror.sync_side_refs(PREFIX, &[], &[]).unwrap();
    assert!(mirror.side_refs(PREFIX).unwrap().is_empty(), "entfernt");
    drop(mirror);
    let reopened = baruch_server::AppState::open(server.path()).await.expect("Neustart auf demselben Datenordner");
    let refs = reopened.mirror(work).await.unwrap().side_refs(PREFIX).unwrap();
    assert!(refs.contains_key(&name), "beim Start nachgeführt: {refs:?}");
}

/// PM close R22 (review): in the side branch relative image paths are from the root as in HEAD, so that `main.typ`
/// including `abschnitte/` can be typeset from a side branch too.
#[tokio::test]
async fn side_refs_write_image_paths_from_the_root_like_head() {
    let server = common::open().await;
    let (work, key, slug) = setup(&server).await;
    let first = body_of(&server, work, 1).await;
    let with_image = format!("{first}\n#image(\"bilder/pegel.svg\")\n");
    let saved = server
        .rpc("outline.save", json!({ "work": work, "key": key, "body": with_image, "based_on": 1, "mode": "new" }))
        .await;
    assert_eq!(result(&saved)["rev"].as_u64(), Some(2), "Fassung 2 mit Bild");
    let saved = server
        .rpc("outline.save", json!({ "work": work, "key": key, "body": first, "based_on": 2, "mode": "new" }))
        .await;
    assert_eq!(result(&saved)["rev"].as_u64(), Some(3), "Fassung 3 ohne Bild, Fassung 2 wird Seitenzweig");
    let mirror = server.state.mirror(work).await.unwrap();
    let path = std::fs::read_dir(server.mirror_dir_of(&slug).join("abschnitte"))
        .unwrap()
        .map(|e| format!("abschnitte/{}", e.unwrap().file_name().to_string_lossy()))
        .find(|p| p.starts_with("abschnitte/000-"))
        .expect("Abschnittsdatei 000");
    let side = mirror.read_file(&format!("{PREFIX}{key}/2"), &path).unwrap().expect("Fassung 2 im Seitenzweig");
    assert!(side.contains("image(\"/bilder/pegel.svg\")"), "Bildpfad ab der Wurzel wie im HEAD: {side}");
}
