// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 13, task „abgleich“: sync Baruch ↔ MultiGPT (`crate::sync`, migration 0033).
//!
//! - Without pressing a button no request to MultiGPT (not at start, saving or displaying either).
//! - Upload only with the switch (default off, switching on only with confirmation): one document per chapter,
//!   unchanged nothing, changed first upload anew and then delete the old one, deleted chapters and works clear away
//!   their documents; without `docs.write` a hint and locally no change.
//! - Fetching chosen collections only on a change, with limits and hints; afterwards the local search finds the
//!   document as kind `remote` with a link to the document view; `local_document` for the integrator.
//! - Page `/abgleich` and Einstellungen → Werk without JavaScript; no MCP tool for the sync.
//!
//! Peers are only mocks on 127.0.0.1:0 ([`MockDocs`] for MultiGPT, [`MockEliezer`] for the embeddings). Texts freely
//! invented („Die Insel der Bienen“).

use crate::common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::MockEliezer;
use baruch_server::rag;
use baruch_server::sync::mock::{Deny, MockDocs};
use baruch_server::sync::{self, Limits};
use baruch_store::SyncSide;
use serde_json::{Value, json};

const ISLAND: &str = "// Testbuch Runde 13 „abgleich“ (frei erfunden).
#set document(title: \"Die Insel der Bienen\", author: \"Ole Wabe\")

= Ankunft

Das Boot legte am Steg an, und die Luft summte über den Dünen.

== Der Imker

Der alte Imker trug einen Hut mit Schleier und zählte die Stöcke.

= Der Sommer

Die Bienen flogen zum Lavendel auf der Westseite der Insel.

= Der Abschied

Im Herbst wurden die Stöcke winterfest gemacht und mit Stroh umwickelt.
";

const COLLECTION: &str = "Baruch: Die Insel der Bienen";

fn result(answer: &Value) -> &Value {
    answer.get("result").unwrap_or_else(|| panic!("Fehler statt Ergebnis: {answer}"))
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

/// Server with a work of its own and the mock as MultiGPT (address entered, without token).
async fn island() -> (common::Server, MockDocs, i64) {
    let server = common::open().await;
    let mock = MockDocs::start().await;
    let set = server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": mock.url() } })).await;
    result(&set);
    let work = server.import(ISLAND, "insel.typ").await.work;
    (server, mock, work)
}

/// Start a run, wait and deliver the last run from `sync.status`.
async fn run(server: &common::Server) -> Value {
    let started = server.rpc("sync.run", json!({})).await;
    assert_eq!(result(&started)["started"], true, "kein anderer Lauf läuft, dieser startet: {started}");
    sync::wait_idle(&server.state).await;
    let status = server.rpc("sync.status", json!({})).await;
    let last = result(&status)["last"].clone();
    assert!(last["finished"].is_string(), "der Lauf ist beendet: {status}");
    last
}

async fn upload_on(server: &common::Server, work: i64) {
    let on = server.rpc("sync.set_upload", json!({ "work": work, "on": true, "confirm": true })).await;
    assert_eq!(result(&on)["upload"], true, "eingeschaltet: {on}");
}

/// Nodes of a work: (key, title, text of the current revision, number of the current revision).
async fn nodes(server: &common::Server, work: i64) -> Vec<(i64, String, String, i64)> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let current = n["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
            (
                n["key"].as_i64().unwrap(),
                n["title"].as_str().unwrap().to_owned(),
                current["body"].as_str().unwrap().to_owned(),
                current["number"].as_i64().unwrap(),
            )
        })
        .collect()
}

fn tool_names(mock: &MockDocs) -> Vec<String> {
    mock.requests()
        .iter()
        .filter(|r| r["method"] == "tools/call")
        .map(|r| r["params"]["name"].as_str().unwrap_or("").to_owned())
        .collect()
}

#[tokio::test]
async fn without_pressing_the_button_nothing_reaches_multigpt_not_even_at_start_or_when_saving() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    let chosen = server.rpc("sync.set_collections", json!({ "collections": ["Bibeln"] })).await;
    assert_eq!(result(&chosen)["collections"], json!(["Bibeln"]), "Sammlung gewählt: {chosen}");
    let list = nodes(&server, work).await;
    let (key, _, body, rev) = &list[2];
    let saved = server
        .rpc(
            "outline.save",
            json!({ "work": work, "key": key, "body": body.replace("Lavendel", "Thymian"), "based_on": rev }),
        )
        .await;
    result(&saved);
    let status = server.rpc("sync.status", json!({ "work": work })).await;
    assert_eq!(result(&status)["last"], Value::Null, "noch kein Lauf: {status}");
    for uri in [format!("/abgleich?work={work}"), format!("/einstellungen/werk?work={work}")] {
        let (code, html) = server.get(&uri).await;
        assert_eq!(code, StatusCode::OK, "{uri} lädt: {html}");
    }
    // Restart: a second AppState on the same data folder (as when the service starts up).
    let again = baruch_server::AppState::open(server.path()).await.expect("Neustart");
    assert!(!sync::running(&again), "beim Start läuft kein Abgleich");
    assert!(
        mock.requests().is_empty(),
        "Begründung: Nur der Knopf löst den Abgleich aus – Schalter, Sammlungen, Speichern, Seiten, Status und \
         Neustart schicken nichts an MultiGPT: {:?}",
        mock.requests()
    );
}

#[tokio::test]
async fn uploading_is_off_by_default_and_switching_it_on_needs_a_confirmation() {
    let (server, mock, work) = island().await;
    let status = server.rpc("sync.status", json!({ "work": work })).await;
    let entry = &result(&status)["works"][0];
    assert_eq!((entry["work"].as_i64(), &entry["upload"]), (Some(work), &json!(false)), "Vorgabe aus: {status}");
    let refused = server.rpc("sync.set_upload", json!({ "work": work, "on": true })).await;
    assert_eq!(error_code(&refused), -32602, "ohne confirm abgelehnt (Bestätigung vor dem ersten Hochladen)");
    assert!(refused["error"]["message"].as_str().unwrap().contains("bestätigen"), "Satz sagt warum: {refused}");
    let refused = server.rpc("sync.set_upload", json!({ "work": work, "on": true, "confirm": false })).await;
    assert_eq!(error_code(&refused), -32602, "confirm: false gilt nicht als Bestätigung");
    let unknown = server.rpc("sync.set_upload", json!({ "work": 99999, "on": true, "confirm": true })).await;
    assert_eq!(error_code(&unknown), -32602, "unbekanntes Werk");

    let last = run(&server).await;
    assert!(tool_names(&mock).iter().all(|n| n != "upload_document"), "ohne Schalter lädt nichts hoch");
    assert!(
        mock.requests().is_empty(),
        "kein Werk mit Schalter und keine Sammlung: der Lauf fragt MultiGPT gar nicht: {:?}",
        mock.requests()
    );
    assert_eq!(last["ok"], true, "nichts zu tun ist kein Fehler: {last}");
    assert!(last["message"].as_str().unwrap().contains("Nichts abzugleichen"), "ehrlicher Satz: {last}");

    let on = server.rpc("sync.set_upload", json!({ "work": work, "on": true, "confirm": true })).await;
    assert_eq!(result(&on)["collection"], COLLECTION, "Sammlung „Baruch: <Werktitel>“ festgehalten: {on}");
    let off = server.rpc("sync.set_upload", json!({ "work": work, "on": false })).await;
    assert_eq!(result(&off)["upload"], false, "Ausschalten braucht keine Bestätigung: {off}");
    assert_eq!(result(&off)["collection"], COLLECTION, "die Sammlung bleibt festgehalten: {off}");
}

#[tokio::test]
async fn each_chapter_becomes_one_document_and_only_changes_are_uploaded_again() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    let last = run(&server).await;
    assert_eq!(last["ok"], true, "erster Lauf ohne Fehler: {last}");
    assert_eq!(last["uploaded"], 3, "drei Kapitel → drei Dokumente: {last}");
    let uploads = mock.tool_calls("upload_document");
    assert_eq!(uploads.len(), 3, "ein Aufruf je Kapitel: {uploads:?}");
    assert!(uploads.iter().all(|a| a["collection"] == COLLECTION), "alle in „Baruch: <Werk>“: {uploads:?}");
    let titles: Vec<&str> = uploads.iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["1 Ankunft", "2 Der Sommer", "3 Der Abschied"], "Titel „<Nummer> <Titel>“");
    let first = uploads[0]["content"].as_str().unwrap();
    assert!(
        first.contains("Das Boot legte") && first.contains("Der alte Imker"),
        "Kapitel mit Unterabschnitt in einem Dokument (aktuelle Fassungen): {first}"
    );
    assert!(!first.contains("Lavendel"), "das nächste Kapitel gehört nicht dazu: {first}");
    assert_eq!(mock.documents(COLLECTION).len(), 3, "drei Dokumente bei MultiGPT");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_eq!(states.len(), 3, "je Kapitel ein Stand in sync_state: {states:?}");
    let keys = nodes(&server, work).await;
    assert_eq!(states[0].item, format!("work:{work}/key:{}", keys[0].0), "item = work:<W>/key:<K>: {states:?}");
    assert!(states.iter().all(|s| s.hash.len() == 64 && s.remote_id.starts_with("doc-")), "Hash und ID gemerkt");

    // Second run without a change: nothing touched.
    mock.clear_requests();
    let last = run(&server).await;
    let calls = tool_names(&mock);
    assert!(
        !calls.iter().any(|n| n == "upload_document" || n == "delete_document"),
        "Begründung: Unveränderte Kapitel (gleicher Hash in sync_state) werden nicht angefasst: {calls:?}"
    );
    assert_eq!((last["unchanged"].as_i64(), last["uploaded"].as_i64()), (Some(3), Some(0)), "Zahlen: {last}");

    // A change in the subsection: exactly this chapter anew, afterwards the old document deleted.
    let old_id = states[0].remote_id.clone();
    let (key, _, body, rev) = &keys[1];
    let saved = server
        .rpc(
            "outline.save",
            json!({ "work": work, "key": key, "body": body.replace("Schleier", "Netz"), "based_on": rev }),
        )
        .await;
    result(&saved);
    mock.clear_requests();
    let last = run(&server).await;
    let writes: Vec<(String, Value)> = mock
        .requests()
        .into_iter()
        .filter(|r| r["method"] == "tools/call")
        .map(|r| (r["params"]["name"].as_str().unwrap().to_owned(), r["params"]["arguments"].clone()))
        .filter(|(n, _)| n == "upload_document" || n == "delete_document")
        .collect();
    assert_eq!(writes.len(), 2, "genau ein Hochladen und ein Löschen: {writes:?}");
    assert_eq!(writes[0].0, "upload_document", "erst neu hochladen …");
    assert_eq!(writes[0].1["title"], "1 Ankunft", "… genau das geänderte Kapitel …");
    assert!(writes[0].1["content"].as_str().unwrap().contains("Netz"), "… mit dem neuen Text");
    assert_eq!(writes[1], ("delete_document".to_owned(), json!({ "document_id": old_id })), "dann das alte löschen");
    assert_eq!((last["replaced"].as_i64(), last["unchanged"].as_i64()), (Some(1), Some(2)), "Zahlen: {last}");
    let docs = mock.documents(COLLECTION);
    assert_eq!(docs.len(), 3, "weiter drei Dokumente: {docs:?}");
    assert!(!docs.iter().any(|d| d.id == old_id), "das alte Dokument ist weg");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_ne!(states[0].remote_id, old_id, "sync_state zeigt auf das neue Dokument: {states:?}");

    // A deleted chapter deletes its document.
    let gone = keys[3].0;
    let deleted = server.rpc("outline.delete", json!({ "work": work, "key": gone, "confirm": true })).await;
    result(&deleted);
    let gone_id = states.iter().find(|s| s.node_key == Some(gone)).unwrap().remote_id.clone();
    mock.clear_requests();
    let last = run(&server).await;
    assert_eq!(mock.tool_calls("delete_document"), vec![json!({ "document_id": gone_id })], "sein Dokument gelöscht");
    assert!(mock.tool_calls("upload_document").is_empty(), "sonst nichts hochgeladen");
    assert_eq!(last["removed"], 1, "ein Dokument entfernt: {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 2, "zwei Kapitel, zwei Dokumente");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_eq!(states.len(), 2, "sein Stand ist entfernt: {states:?}");
}

#[tokio::test]
async fn documents_of_a_deleted_work_are_removed_by_the_next_run() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    run(&server).await;
    assert_eq!(mock.documents(COLLECTION).len(), 3, "hochgeladen");
    let deleted = server.rpc("work.delete", json!({ "work": work, "confirm_title": "Die Insel der Bienen" })).await;
    result(&deleted);
    let status = server.rpc("sync.status", json!({})).await;
    assert!(
        !result(&status)["works"].as_array().unwrap().iter().any(|w| w["work"] == work),
        "die sync_work-Zeile ging mit dem Werk: {status}"
    );
    let last = run(&server).await;
    assert_eq!(mock.tool_calls("delete_document").len(), 3, "alle drei Dokumente gelöscht");
    assert_eq!((last["removed"].as_i64(), last["ok"].as_bool()), (Some(3), Some(true)), "Zahlen: {last}");
    assert!(mock.documents(COLLECTION).is_empty(), "bei MultiGPT aufgeräumt");
    let states = server.state.store.sync_states(SyncSide::Baruch, None).await.unwrap();
    assert!(states.is_empty(), "keine Stände mehr: {states:?}");
}

#[tokio::test]
async fn without_docs_write_the_run_ends_with_a_hint_and_nothing_changes_locally() {
    for deny in [Deny::Forbidden, Deny::ToolError] {
        let (server, mock, work) = island().await;
        upload_on(&server, work).await;
        mock.deny_write(Some(deny));
        let before = server.rpc("work.export", json!({ "work": work })).await;
        let last = run(&server).await;
        assert_eq!(last["ok"], false, "{deny:?}: der Lauf endet mit ok=0: {last}");
        let message = last["message"].as_str().unwrap();
        assert!(message.contains("docs.write"), "{deny:?}: verständlicher Hinweis nennt das fehlende Recht: {message}");
        assert_eq!((last["failed"].as_i64(), last["uploaded"].as_i64()), (Some(1), Some(0)), "{deny:?}: {last}");
        assert_eq!(
            mock.tool_calls("upload_document").len(),
            1,
            "{deny:?}: nach dem ersten Fehler kein weiterer Versuch"
        );
        assert!(mock.documents(COLLECTION).is_empty(), "{deny:?}: nichts bei MultiGPT");
        let states = server.state.store.sync_states(SyncSide::Baruch, None).await.unwrap();
        assert!(states.is_empty(), "{deny:?}: kein Stand gespeichert (nächster Lauf versucht es neu): {states:?}");
        let after = server.rpc("work.export", json!({ "work": work })).await;
        assert_eq!(after, before, "{deny:?}: der Buchtext ist unverändert");
        // The work goes on: saving succeeds.
        let list = nodes(&server, work).await;
        let (key, _, body, rev) = &list[2];
        let saved = server
            .rpc(
                "outline.save",
                json!({ "work": work, "key": key, "body": format!("{body}\nNoch ein Satz.\n"), "based_on": rev }),
            )
            .await;
        result(&saved);
        let html = server.get(&format!("/abgleich?work={work}")).await.1;
        assert!(html.contains("docs.write"), "{deny:?}: die Seite zeigt den Hinweis: {html}");
        assert!(html.contains("mit Hinweisen"), "{deny:?}: und dass der Lauf nicht fehlerfrei war");
    }
}

#[tokio::test]
async fn offline_the_run_reports_unreachable_and_a_second_concurrent_run_says_running() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    mock.configure(|b| b.delay = Duration::from_millis(400));
    let first = server.rpc("sync.run", json!({})).await;
    assert_eq!(result(&first)["started"], true, "erster Lauf startet: {first}");
    let second = server.rpc("sync.run", json!({})).await;
    assert_eq!(
        (result(&second)["started"].as_bool(), result(&second)["running"].as_bool()),
        (Some(false), Some(true)),
        "Begründung: ein Lauf zur Zeit – der zweite meldet running statt doppelt hochzuladen: {second}"
    );
    sync::wait_idle(&server.state).await;
    assert_eq!(mock.tool_calls("upload_document").len(), 3, "nur der erste Lauf lud hoch");

    // Peer gone (address on a closed port): hint, ok=0, locally everything stays.
    let closed = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let gone = format!("http://127.0.0.1:{closed}/mcp");
    result(&server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": gone } })).await);
    let list = nodes(&server, work).await;
    let (key, _, body, rev) = &list[2];
    let saved = server
        .rpc(
            "outline.save",
            json!({ "work": work, "key": key, "body": body.replace("Westseite", "Ostseite"), "based_on": rev }),
        )
        .await;
    result(&saved);
    mock.clear_requests();
    let last = run(&server).await;
    assert_eq!(last["ok"], false, "offline: ok=0: {last}");
    assert!(last["message"].as_str().unwrap().contains("nicht erreichbar"), "Hinweis „nicht erreichbar“: {last}");
    assert_eq!(last["failed"], 1, "ein Fehler gezählt: {last}");
    let states = server.state.store.sync_states(SyncSide::Baruch, None).await.unwrap();
    assert_eq!(states.len(), 3, "die Stände bleiben (der nächste Lauf lädt das Kapitel nach): {states:?}");
    assert!(mock.requests().is_empty(), "nichts ging an die alte Adresse");
}

#[tokio::test]
async fn chosen_collections_are_fetched_only_when_changed_and_then_found_offline_as_remote() {
    let (server, mock, work) = island().await;
    let eliezer = MockEliezer::start().await;
    result(&server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await);
    rag::set_embedders(&server.state, rag::eliezer_embedders(&server.state));
    rag::catch_up(&server.state).await;
    rag::wait_idle(&server.state).await;
    mock.put_document(
        "Leuchtfeuer",
        "lf-1",
        "Das Feuer auf der Klippe",
        "Der Wärter putzte jeden Abend die Linse der Laterne.",
    );
    mock.set_url("lf-1", "https://beispiel.invalid/feuer");
    mock.put_document("Leuchtfeuer", "lf-2", "Die Glocke im Nebel", "Bei Nebel läutete die Glocke im Takt der Wellen.");
    mock.put_document("Andere", "x-1", "Fremd", "Nicht gewählt.");
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Leuchtfeuer"] })).await);

    let last = run(&server).await;
    assert_eq!((last["downloaded"].as_i64(), last["ok"].as_bool()), (Some(2), Some(true)), "zwei geholt: {last}");
    assert_eq!(mock.tool_calls("read_document").len(), 2, "je Dokument ein read_document");
    assert!(
        mock.tool_calls("list_documents").iter().all(|a| a["collection"] == "Leuchtfeuer"),
        "nur die gewählte Sammlung gelistet: {:?}",
        mock.tool_calls("list_documents")
    );
    assert!(mock.tool_calls("upload_document").is_empty(), "Holen lädt nichts hoch");
    let copy = sync::local_document(&server.state, "Leuchtfeuer", "lf-1").await.unwrap().expect("lokale Kopie");
    assert_eq!(copy.title, "Das Feuer auf der Klippe", "Titel aus list_documents");
    assert_eq!(copy.url.as_deref(), Some("https://beispiel.invalid/feuer"), "Adresse übernommen");
    assert!(copy.text.contains("Linse der Laterne"), "Volltext: {copy:?}");
    assert!(copy.fetched.starts_with("20"), "Abrufzeit: {copy:?}");
    assert!(sync::local_document(&server.state, "Leuchtfeuer", "gibt-es-nicht").await.unwrap().is_none(), "sonst None");
    assert!(sync::local_document(&server.state, "Andere", "x-1").await.unwrap().is_none(), "nicht gewählt: None");
    assert!(
        sync::local_document(&server.state, "Leuchtfeuer", "lf-2").await.unwrap().unwrap().url.is_none(),
        "ohne URL"
    );

    // Second run: modification times equal → nothing read.
    mock.clear_requests();
    let last = run(&server).await;
    assert!(
        mock.tool_calls("read_document").is_empty(),
        "Begründung: list_documents meldet dieselbe Änderungszeit (remote_stamp) – nichts wird erneut gelesen"
    );
    assert_eq!((last["unchanged"].as_i64(), last["downloaded"].as_i64()), (Some(2), Some(0)), "Zahlen: {last}");

    // A document changes at MultiGPT: exactly this one is read.
    mock.put_document(
        "Leuchtfeuer",
        "lf-2",
        "Die Glocke im Nebel",
        "Bei Nebel läutete die Glocke dreimal hintereinander.",
    );
    mock.clear_requests();
    let last = run(&server).await;
    assert_eq!(mock.tool_calls("read_document"), vec![json!({ "document_id": "lf-2" })], "nur das geänderte gelesen");
    assert_eq!(last["downloaded"], 1, "ein Dokument neu geholt: {last}");

    // Without a modification time: it is read, but the same hash changes nothing.
    mock.configure(|b| b.stamps = false);
    mock.clear_requests();
    let last = run(&server).await;
    assert_eq!(mock.tool_calls("read_document").len(), 2, "ohne remote_stamp entscheidet der Hash des Texts");
    assert_eq!((last["downloaded"].as_i64(), last["unchanged"].as_i64()), (Some(0), Some(2)), "Zahlen: {last}");

    // Searchable offline: the local index has the document as kind „remote“ with origin and link.
    rag::wait_idle(&server.state).await;
    let found = server
        .rpc(
            "rag.search",
            json!({ "work": work, "query": "Wärter putzte die Linse der Laterne", "collections": ["remote"] }),
        )
        .await;
    let hits = result(&found)["hits"].as_array().unwrap().clone();
    let hit = hits.iter().find(|h| h["meta"]["document"] == "lf-1").unwrap_or_else(|| panic!("Treffer fehlt: {found}"));
    assert_eq!((&hit["way"], &hit["collection"]), (&json!("lancedb"), &json!("remote")), "Art remote: {hit}");
    assert_eq!(hit["url"], "/quelle/multigpt?collection=Leuchtfeuer&document=lf-1", "Link zur Dokumentansicht: {hit}");
    assert_eq!(hit["document"], "remote:Leuchtfeuer/lf-1", "Dokument mit Herkunft: {hit}");
    assert_eq!(hit["meta"]["collection"], "Leuchtfeuer", "meta.collection: {hit}");
    assert!(hit["meta"]["sync"].is_i64(), "meta.sync = id der Kopie: {hit}");
    assert!(eliezer.requests().iter().all(|r| r.path == "/v1/embeddings"), "nur Einbettungen, kein Chat");

    // Deleted at MultiGPT → removed locally; deselected → removed locally.
    mock.remove_document("lf-2");
    let last = run(&server).await;
    assert_eq!(last["removed"], 1, "die Kopie des gelöschten Dokuments entfernt: {last}");
    assert!(sync::local_document(&server.state, "Leuchtfeuer", "lf-2").await.unwrap().is_none(), "lf-2 weg");
    result(&server.rpc("sync.set_collections", json!({ "collections": [] })).await);
    mock.clear_requests();
    let last = run(&server).await;
    assert!(sync::local_document(&server.state, "Leuchtfeuer", "lf-1").await.unwrap().is_none(), "abgewählt: weg");
    assert_eq!(last["removed"], 1, "eine Kopie entfernt: {last}");
    assert!(mock.requests().is_empty(), "Aufräumen braucht kein Netz: {:?}", mock.requests());
}

#[tokio::test]
async fn the_limits_for_documents_size_and_run_apply_with_a_hint() {
    let (server, mock, _) = island().await;
    sync::set_limits(
        &server.state,
        Limits { documents: 3, document_bytes: 100, run_bytes: 250, call_timeout: Duration::from_secs(30) },
    );
    for (id, size) in [("g1", 80), ("g2", 150), ("g3", 80), ("g4", 80), ("g5", 80)] {
        mock.put_document("Gross", id, id, &"a".repeat(size));
    }
    for id in ["k1", "k2"] {
        mock.put_document("Klein", id, id, &"b".repeat(80));
    }
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Gross", "Klein"] })).await);
    let last = run(&server).await;
    let message = last["message"].as_str().unwrap();
    assert!(message.contains("mehr als 3 Dokumente"), "Grenze je Sammlung mit Hinweis: {message}");
    assert!(message.contains("„g2“ (Gross) ist größer als 100 Bytes"), "Grenze je Dokument mit Hinweis: {message}");
    assert!(message.contains("Grenze von 250 Bytes je Abgleich"), "Grenze je Lauf mit Hinweis: {message}");
    assert_eq!(last["ok"], false, "Grenzen sind Hinweise, der Lauf ist nicht fehlerfrei: {last}");
    assert_eq!(last["downloaded"], 3, "g1, g3 (die ersten drei ohne g2) und k1 (danach ist das Budget voll): {last}");
    let ids: Vec<(String, String)> = server.state.store.sync_document_ids().await.unwrap();
    assert_eq!(
        ids,
        vec![("Gross".into(), "g1".into()), ("Gross".into(), "g3".into()), ("Klein".into(), "k1".into())],
        "gespeichert ist genau, was in die Grenzen passt"
    );
    assert!(!mock.tool_calls("read_document").iter().any(|a| a["document_id"] == "g4"), "g4 liegt jenseits von 3");
}

#[tokio::test]
async fn a_document_over_two_mib_is_not_fetched_with_the_default_limits() {
    let (server, mock, _) = island().await;
    assert_eq!(Limits::DEFAULT.document_bytes, 2 * 1024 * 1024, "Vorgabe 2 MiB je Dokument");
    mock.put_document("Archiv", "gross", "Großes Dokument", &"x".repeat(2 * 1024 * 1024 + 1));
    mock.put_document("Archiv", "klein", "Kleines Dokument", "Ein kurzer erfundener Text.");
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Archiv"] })).await);
    let last = run(&server).await;
    assert!(
        last["message"].as_str().unwrap().contains("„Großes Dokument“ (Archiv) ist größer als 2 MiB"),
        "Hinweis zur Grenze: {last}"
    );
    assert_eq!((last["downloaded"].as_i64(), last["failed"].as_i64()), (Some(1), Some(1)), "Zahlen: {last}");
    assert!(sync::local_document(&server.state, "Archiv", "gross").await.unwrap().is_none(), "nicht gespeichert");
    assert!(sync::local_document(&server.state, "Archiv", "klein").await.unwrap().is_some(), "das kleine schon");
}

#[tokio::test]
async fn the_sync_page_works_without_javascript() {
    let (server, mock, work) = island().await;
    let (code, html) = server.get(&format!("/abgleich?work={work}")).await;
    assert_eq!(code, StatusCode::OK, "Seite lädt: {html}");
    // Round 33: the header is the ribbon (header.band instead of header.topbar)
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "eine Kopfzeile aus base.html");
    assert!(html.contains("<h1 class=\"pane-title\">Abgleich mit MultiGPT</h1>"), "Überschrift: {html}");
    assert!(html.contains("<input type=\"hidden\" name=\"action\" value=\"run\">"), "Formular action=run: {html}");
    assert!(html.contains(">Abgleichen</button>"), "Knopf „Abgleichen“: {html}");
    assert!(html.contains("Noch nie abgeglichen."), "noch kein Lauf: {html}");
    assert!(html.contains("<strong>Die Insel der Bienen</strong>"), "Werk in der Liste: {html}");
    assert!(html.contains("data-sync-upload=\"aus\""), "Vorgabe aus: {html}");
    assert!(
        html.contains("<input type=\"checkbox\" name=\"confirm\" value=\"1\" required>"),
        "Pflicht-Kontrollkästchen zum Einschalten: {html}"
    );
    assert!(html.contains("name=\"collection_new\""), "Feld für eine Sammlung: {html}");
    assert!(html.contains("value=\"load_collections\""), "Knopf zum Laden der Sammlungen: {html}");
    assert!(!html.contains("data-sync-unconfigured"), "MultiGPT ist eingerichtet, kein Hinweis darauf");
    assert!(mock.requests().is_empty(), "die Seite fragt MultiGPT nicht");

    // Switching on without the checkbox: rejected, with it: switched on.
    let (code, location) = server.post_form("/abgleich/act", &format!("action=upload&work={work}&on=1")).await;
    assert_eq!(code, StatusCode::SEE_OTHER, "303");
    assert_eq!(location.as_deref(), Some(format!("/abgleich?work={work}&fehler=bestaetigen").as_str()), "Grund");
    let html = server.get(&format!("/abgleich?work={work}&fehler=bestaetigen")).await.1;
    assert!(html.contains("Bitte bestätigen"), "Meldung auf der Seite: {html}");
    assert!(!server.state.store.sync_work(work).await.unwrap().upload, "ohne Kästchen bleibt es aus");
    let (_, location) = server.post_form("/abgleich/act", &format!("action=upload&work={work}&on=1&confirm=1")).await;
    assert_eq!(location.as_deref(), Some(format!("/abgleich?work={work}&ok=hochladen_an").as_str()), "eingeschaltet");
    assert!(server.state.store.sync_work(work).await.unwrap().upload, "Schalter gespeichert");

    // Choosing collections (repeated field and new field), loading at the press of a button.
    mock.put_document("Bibeln", "b-1", "Psalm der Wellen", "Erfundener Psalm.");
    let (_, location) = server
        .post_form(
            "/abgleich/act",
            &format!("action=collections&work={work}&collection=Bibeln&collection_new=Kommentare"),
        )
        .await;
    assert_eq!(location.as_deref(), Some(format!("/abgleich?work={work}&ok=sammlungen").as_str()), "gespeichert");
    assert_eq!(server.state.store.sync_collections().await.unwrap(), vec!["Bibeln", "Kommentare"], "beide gewählt");
    let (_, location) = server.post_form("/abgleich/act", "action=collections&collection_new=a%0Ab").await;
    assert_eq!(location.as_deref(), Some("/abgleich?fehler=sammlung"), "ungültige Sammlung abgelehnt");
    let (_, location) = server.post_form("/abgleich/act", &format!("action=load_collections&work={work}")).await;
    assert_eq!(location.as_deref(), Some(format!("/abgleich?work={work}&ok=geladen").as_str()), "geladen");
    assert_eq!(tool_names(&mock), vec!["list_collections"], "nur list_collections auf Knopfdruck");
    let html = server.get(&format!("/abgleich?work={work}")).await.1;
    assert!(
        html.contains("<input type=\"checkbox\" name=\"collection\" value=\"Bibeln\" checked> Bibeln"),
        "gewählte Sammlung angehakt: {html}"
    );

    // Syncing through the form: 303, afterwards the result with numbers on the page.
    let (code, location) = server.post_form("/abgleich/act", &format!("action=run&work={work}")).await;
    assert_eq!(
        (code, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/abgleich?work={work}&ok=gestartet").as_str())),
        "ohne JavaScript leitet das Formular mit festem Meldecode auf die Seite zurück"
    );
    sync::wait_idle(&server.state).await;
    let html = server.get(&format!("/abgleich?work={work}")).await.1;
    assert!(
        html.split("zuletzt abgeglichen ").nth(1).is_some_and(|rest| {
            rest.as_bytes()[..10]
                .iter()
                .enumerate()
                .all(|(i, c)| if i == 2 || i == 5 { *c == b'.' } else { c.is_ascii_digit() })
                && rest[10..].starts_with(' ')
                && rest.as_bytes()[11..16]
                    .iter()
                    .enumerate()
                    .all(|(i, c)| if i == 2 { *c == b':' } else { c.is_ascii_digit() })
                && !rest[..20].contains("UTC")
        }),
        "„zuletzt abgeglichen TT.MM.JJJJ HH:MM“ in Ortszeit (Nutzer 2026-11-07) – lesbare Zeit statt ISO-Stempel: {html}"
    );
    assert!(html.contains("hochgeladen: <span data-count=\"uploaded\">3</span>"), "Zahl aus sync_run: {html}");
    assert!(html.contains("geholt: <span data-count=\"downloaded\">1</span>"), "geholt aus Bibeln: {html}");
    assert!(html.contains("data-sync-upload=\"an\""), "Schalter an: {html}");

    // Foreign page: rejected.
    let request = Request::post("/abgleich/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .header(header::HOST, "127.0.0.1:8765")
        .body(Body::from("action=run"))
        .unwrap();
    let (code, _, _) = server.send(request).await;
    assert_eq!(code, StatusCode::FORBIDDEN, "POST /abgleich/act liegt hinter guard::same_origin");
}

#[tokio::test]
async fn settings_work_shows_the_upload_switch_off_by_default() {
    let (server, mock, work) = island().await;
    let (code, html) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(code, StatusCode::OK, "Einstellungen → Werk lädt: {html}");
    let block = &html[html.find("settings-sync").unwrap_or_else(|| panic!("Block fehlt: {html}"))..];
    assert!(block.contains("<form method=\"post\" action=\"/abgleich/act\">"), "Formular an /abgleich/act: {block}");
    assert!(block.contains("data-sync-upload=\"aus\""), "Vorgabe aus: {block}");
    assert!(block.contains("name=\"confirm\" value=\"1\" required"), "Pflicht-Kontrollkästchen: {block}");
    assert!(block.contains(COLLECTION), "nennt die Sammlung „Baruch: <Werk>“ vorab: {block}");
    let back = format!("/einstellungen/werk?work={work}");
    let (_, location) = server
        .post_form(
            "/abgleich/act",
            &format!("action=upload&work={work}&on=1&confirm=1&back=%2Feinstellungen%2Fwerk%3Fwork%3D{work}"),
        )
        .await;
    assert_eq!(location, Some(format!("{back}&ok=hochladen_an")), "zurück in die Einstellungen");
    let html = server.get(&format!("{back}&ok=hochladen_an")).await.1;
    assert!(html.contains("Hochladen nach MultiGPT ist eingeschaltet"), "Hinweis: {html}");
    assert!(html.contains("data-sync-upload=\"an\""), "jetzt an: {html}");
    let (_, location) =
        server.post_form("/abgleich/act", &format!("action=upload&work={work}&back=http%3A%2F%2Ffremd.example")).await;
    assert_eq!(location, Some(format!("/abgleich?work={work}&ok=hochladen_aus")), "fremder Rücksprung wird ignoriert");
    assert!(mock.requests().is_empty(), "Schalten fragt MultiGPT nicht");
}

#[tokio::test]
async fn the_tools_menu_links_to_the_sync_page_instead_of_a_stub() {
    let (server, _mock, work) = island().await;
    let html = server.get(&format!("/?work={work}")).await.1;
    assert!(
        html.contains(&format!(
            "<a class=\"btn\" href=\"/abgleich?work={work}\" title=\"Abgleich mit MultiGPT\">Abgleichen</a>"
        )),
        // Round 33 (ribbon): „Abgleichen“ was in the menu Werkzeuge ▾ and is now in the tab Prüfen (group Quellen) –
        // still an ordinary link, no stub.
        "Reiter Prüfen verlinkt die Seite: {html}"
    );
    let base = include_str!("../templates/base.html");
    assert!(!base.contains("STUB: Abgleich"), "kein STUB-Vermerk mehr in base.html");
    assert!(!base.contains("Abgleich mit MultiGPT folgt (STUB)"), "kein gesperrter Eintrag mehr");
}

#[tokio::test]
async fn there_is_no_mcp_tool_for_the_sync() {
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (code, _, body) = server.send(request).await;
    assert_eq!(code, StatusCode::OK, "tools/list: {body}");
    let list: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(!names.is_empty(), "es gibt Werkzeuge: {list}");
    for name in &names {
        assert!(
            !["sync", "abgleich", "upload", "download"].iter().any(|bad| name.contains(bad)),
            "Begründung: Abgleich und Hochladen löst nur der Mensch aus – kein MCP-Werkzeug {name}"
        );
    }
    let text = list["result"]["tools"].to_string();
    assert!(!text.contains("sync.run") && !text.contains("/abgleich"), "auch keine Beschreibung verweist darauf");
}

#[tokio::test]
async fn an_old_version_whose_deletion_failed_is_deleted_by_the_next_run() {
    let (server, mock, work) = island().await;
    upload_on(&server, work).await;
    run(&server).await;
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    let old_id = states[0].remote_id.clone();
    let keys = nodes(&server, work).await;
    let (key, _, body, rev) = &keys[0];
    let saved = server
        .rpc(
            "outline.save",
            json!({ "work": work, "key": key, "body": body.replace("Boot", "Schiff"), "based_on": rev }),
        )
        .await;
    result(&saved);

    // Deleting fails: the new document is up, the old one stays lying for the time being.
    mock.configure(|b| b.fail_delete = true);
    let last = run(&server).await;
    assert_eq!(last["ok"], false, "der gescheiterte Löschversuch macht den Lauf nicht fehlerfrei: {last}");
    assert!(last["message"].as_str().unwrap().contains(&old_id), "der Hinweis nennt die alte ID: {last}");
    assert_eq!(mock.documents(COLLECTION).len(), 4, "vorübergehend doppelt: drei Kapitel und die alte Fassung");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    // Reason: the old ID must not get lost, otherwise no later run deletes it (replacing instead of doubling).
    assert!(
        states.iter().any(|s| s.remote_id == old_id && s.item.contains("/stale:")),
        "die alte ID bleibt als eigener Stand gemerkt: {states:?}"
    );

    // Next run with working deletion: the chapter is unchanged, the old version is deleted.
    mock.configure(|b| b.fail_delete = false);
    mock.clear_requests();
    let last = run(&server).await;
    assert_eq!(last["ok"], true, "jetzt fehlerfrei: {last}");
    assert_eq!(mock.tool_calls("delete_document"), vec![json!({ "document_id": old_id })], "genau die alte Fassung");
    assert!(mock.tool_calls("upload_document").is_empty(), "nichts erneut hochgeladen (Hash gleich)");
    let docs = mock.documents(COLLECTION);
    assert_eq!(docs.len(), 3, "wieder genau ein Dokument je Kapitel: {docs:?}");
    assert!(docs.iter().any(|d| d.text.contains("Schiff")), "die neue Fassung bleibt");
    let states = server.state.store.sync_states(SyncSide::Baruch, Some(COLLECTION)).await.unwrap();
    assert_eq!(states.len(), 3, "der Stand der alten Fassung ist entfernt: {states:?}");
}

#[tokio::test]
async fn a_document_read_page_by_page_is_stored_whole_like_the_online_fulltext() {
    let (server, mock, _) = island().await;
    mock.configure(|b| {
        b.tools = sync::mock::paged_tools();
        b.page_chars = 40;
    });
    let text = "Erste Seite vom Kai. Die Möwen schreien laut. Zweite Seite: Der Nebel zieht auf. Letzte Seite: Das Feuer brennt.";
    mock.put_document("Hafen", "hf-1", "Hafenbuch", text);
    result(&server.rpc("sync.set_collections", json!({ "collections": ["Hafen"] })).await);
    let last = run(&server).await;
    assert_eq!((last["downloaded"].as_i64(), last["ok"].as_bool()), (Some(1), Some(true)), "geholt: {last}");
    let pages: Vec<Value> = mock.tool_calls("read_document").iter().map(|a| a["page"].clone()).collect();
    // Reason: 112 characters at 40 each → three pages; each is requested exactly once.
    assert_eq!(pages, vec![json!(1), json!(2), json!(3)], "alle Seiten gelesen: {pages:?}");
    let copy = sync::local_document(&server.state, "Hafen", "hf-1").await.unwrap().expect("lokale Kopie");
    assert!(copy.text.contains("Das Feuer brennt"), "auch die letzte Seite steht in der Kopie: {:?}", copy.text);
    // Reason: offline checks (highlights, document view) compare with the copy; it must equal the online full text
    // (read_full, pages joined with an empty line).
    let way = baruch_server::connections::multigpt::MultigptWay::new(
        &mock.url(),
        None,
        baruch_server::connections::multigpt::Limits::SEARCH,
    );
    assert_eq!(copy.text, way.read_full("hf-1").await.expect("Volltext"), "Kopie = Online-Volltext");

    // A change only on the last page changes the hash and is fetched.
    mock.configure(|b| b.stamps = false);
    mock.put_document("Hafen", "hf-1", "Hafenbuch", &text.replace("brennt", "erlischt"));
    let last = run(&server).await;
    assert_eq!(last["downloaded"], 1, "Änderung auf Seite 3 erkannt: {last}");
    let copy = sync::local_document(&server.state, "Hafen", "hf-1").await.unwrap().unwrap();
    assert!(copy.text.contains("erlischt"), "neue letzte Seite: {:?}", copy.text);
}
