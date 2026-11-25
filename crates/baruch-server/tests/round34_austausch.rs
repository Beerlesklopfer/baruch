// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap“ – Ein- und Ausgabe (Besitz mindmap_austausch; in der Schnittstelle R34-S1 nur die S1-Zusage
//! von `idea_map::tree::import`). Werk und Baum sind frei erfunden; kein Modell, kein Netz.

use crate::common;

use baruch_server::idea_map::tree::{self, ImportNode, ImportTree, NOT_YET};
use baruch_store::IdeaOrigin;
use serde_json::json;

/// S1-Zusage: `import` ist ein ehrlicher Stub – er lehnt mit `NOT_YET` ab und legt nichts an.
#[tokio::test]
async fn import_refuses_honestly_and_creates_nothing() {
    let server = common::open().await;
    let work = server.import("#set document(title: \"Fährbuch\")\n\n= Anleger\n\nText.\n", "faehrbuch.typ").await.work;
    let tree = ImportTree {
        title: "Fremde Karte".to_owned(),
        nodes: vec![ImportNode {
            title: "Tau".to_owned(),
            children: vec![ImportNode { title: "Knoten".to_owned(), ..ImportNode::default() }],
            ..ImportNode::default()
        }],
        ..ImportTree::default()
    };
    let origin = IdeaOrigin::Human { name: "Mensch".to_owned() };
    let error = tree::import(&server.state, work, None, &tree, &origin)
        .await
        .expect_err("solange der Baum-Import fehlt, darf import keinen Erfolg melden");
    assert_eq!(error.message, NOT_YET, "die Ablehnung nennt ehrlich, dass die Mindmap noch nicht umgesetzt ist");
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "Ablehnung mit dem Code „abgelehnt“, kein interner Fehler");
    let answer = server.rpc("idea.list", json!({ "work": work })).await;
    assert!(answer["error"].is_null(), "idea.list gelingt: {answer}");
    assert_eq!(
        answer["result"]["ideas"].as_array().map(Vec::len),
        Some(0),
        "der Stub legt keinen Einfall an: {answer}"
    );

    let json = serde_json::to_string(&tree).unwrap();
    let back: ImportTree = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tree, "ImportTree übersteht JSON (verborgenes Formularfeld zwischen Vorschau und Bestätigung)");
}
