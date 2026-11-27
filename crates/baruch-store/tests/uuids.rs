// SPDX-License-Identifier: AGPL-3.0-or-later
//! Globally unique identifiers (Round 26 „ids“, migrations 0041 and 0042) on a real SQLite file in the temp directory,
//! test texts freely invented.
//!
//! Proven:
//! - Every write path of the store assigns new rows a canonical v4 uuid (through the fill triggers) without naming the
//!   column itself.
//! - Moving, renaming, level, number, switching, accepting, changing a revision and switching an image preserve every
//!   existing uuid.
//! - Deleting leaves exactly one tombstone with kind and work uuid per deleted row, also for threads that go with the
//!   work via cascade (0042); soft deletion of an idea leaves none; a tombstone holds up nothing in the store.
//! - The queries through uuid (`work_uuid`, `work_by_uuid`, `node_key_by_uuid`, `node_uuid`) and the name of the
//!   instance.
//! - No code path except `exchange_apply` writes the column uuid (source check).
//! - Round 25 (migration 0043): links (`idea_link`) and positions (`idea_map_pos`) get their uuid likewise through fill
//!   triggers and leave tombstones `idea_link`/`idea_map_pos` with the work uuid, also through the cascade of a
//!   hard-deleted idea or work.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use baruch_core::{Author, Post, TextRef, WorkKind};
use baruch_store::{
    AssistantAdd, IdeaOrigin, ImportAsset, MAX_INSTANCE_NAME_CHARS, NewAsset, NewIdea, NewPoint, NewWork, Place,
    Preset, RevisionEdit, Store, StoreError, WorkEdit, WorkId,
};

const HUMAN: &str = "Anna";

/// Freely invented book with work template, two chapters and one section.
const ORCHARD: &str = "#set document(title: \"Der Obstgarten\", author: \"Anna Apfel\")\n\n\
= Die Bäume\n\nApfel und Birne.\n\n== Der Schnitt\n\nIm Februar.\n\n= Die Ernte\n\nKörbe und Leitern.\n";

/// Freely invented book with an image.
const BARN_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"4\" height=\"3\"/></svg>\n";
const BARN_SOURCE: &str = "#set document(title: \"Die Scheune\")\n\n= Das Dach\n\n#image(\"bilder/scheune.svg\")\n";

/// The ten tables with a uuid and their kind in the tombstone.
const TABLES: [(&str, &str); 10] = [
    ("work", "work"),
    ("node", "node"),
    ("revision", "revision"),
    ("work_template_revision", "template_revision"),
    ("post", "post"),
    ("idea", "idea"),
    ("chat_thread", "thread"),
    ("chat_thread_post", "thread_post"),
    ("asset", "asset"),
    ("asset_revision", "asset_revision"),
];

/// Per table: id, uuid and the uuid of the work to which the row belongs (the way the tombstone trigger determines it).
fn rows_sql(table: &str) -> &'static str {
    match table {
        "work" => "SELECT id, uuid, uuid FROM work",
        "node" => "SELECT n.id, n.uuid, w.uuid FROM node n LEFT JOIN work w ON w.id = n.work_id",
        "revision" => {
            "SELECT r.id, r.uuid, w.uuid FROM revision r LEFT JOIN node n ON n.id = r.node_id \
             LEFT JOIN work w ON w.id = n.work_id"
        }
        "work_template_revision" => {
            "SELECT t.id, t.uuid, w.uuid FROM work_template_revision t LEFT JOIN work w ON w.id = t.work_id"
        }
        "post" => {
            "SELECT p.id, p.uuid, w.uuid FROM post p LEFT JOIN node n ON n.id = p.node_id \
             LEFT JOIN work w ON w.id = n.work_id"
        }
        "idea" => "SELECT i.id, i.uuid, w.uuid FROM idea i LEFT JOIN work w ON w.id = i.work_id",
        "chat_thread" => "SELECT t.id, t.uuid, w.uuid FROM chat_thread t LEFT JOIN work w ON w.id = t.work_id",
        "chat_thread_post" => {
            "SELECT p.id, p.uuid, w.uuid FROM chat_thread_post p LEFT JOIN chat_thread t ON t.id = p.thread_id \
             LEFT JOIN work w ON w.id = t.work_id"
        }
        "asset" => "SELECT a.id, a.uuid, w.uuid FROM asset a LEFT JOIN work w ON w.id = a.work_id",
        "asset_revision" => {
            "SELECT r.id, r.uuid, w.uuid FROM asset_revision r LEFT JOIN asset a ON a.id = r.asset_id \
             LEFT JOIN work w ON w.id = a.work_id"
        }
        other => panic!("unbekannte Tabelle {other}"),
    }
}

/// One row with a uuid: kind (for the tombstone), uuid, work uuid.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    kind: &'static str,
    uuid: Option<String>,
    work_uuid: Option<String>,
}

type Snapshot = BTreeMap<(&'static str, i64), Row>;

async fn snapshot(store: &Store) -> Snapshot {
    let mut out = BTreeMap::new();
    for (table, kind) in TABLES {
        let rows: Vec<(i64, Option<String>, Option<String>)> =
            sqlx::query_as(rows_sql(table)).fetch_all(store.pool()).await.unwrap();
        for (id, uuid, work_uuid) in rows {
            out.insert((table, id), Row { kind, uuid, work_uuid });
        }
    }
    out
}

async fn tombstones(store: &Store) -> BTreeMap<String, (String, Option<String>)> {
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT uuid, kind, work_uuid FROM uuid_tombstone").fetch_all(store.pool()).await.unwrap();
    rows.into_iter().map(|(u, k, w)| (u, (k, w))).collect()
}

/// Canonical lower case 8-4-4-4-12, version 4, variant 10 (RFC 9562).
fn is_v4(uuid: &str) -> bool {
    baruch_exchange::is_uuid(uuid) && &uuid[14..15] == "4" && matches!(&uuid[19..20], "8" | "9" | "a" | "b")
}

/// Every row of every table has a canonical v4 uuid, and none occurs twice (not across tables either).
fn assert_all_v4(snap: &Snapshot, step: &str) {
    let mut seen = BTreeSet::new();
    for ((table, id), row) in snap {
        let uuid = row.uuid.as_deref().unwrap_or_else(|| panic!("{step}: {table} {id} ohne uuid"));
        assert!(is_v4(uuid), "{step}: {table} {id} hat keine kanonische v4-uuid: {uuid}");
        assert!(seen.insert(uuid.to_owned()), "{step}: uuid {uuid} doppelt ({table} {id})");
    }
}

/// All rows of `before` are in `after` with the same uuid (nothing deleted, nothing rewritten).
fn assert_kept(before: &Snapshot, after: &Snapshot, step: &str) {
    for (key, row) in before {
        let now = after.get(key).unwrap_or_else(|| panic!("{step}: Zeile {key:?} verschwunden"));
        assert_eq!(now.uuid, row.uuid, "{step}: uuid von {key:?} hat sich geändert");
    }
}

/// New rows in `table` between two states.
fn added(before: &Snapshot, after: &Snapshot, table: &str) -> usize {
    after.keys().filter(|key| key.0 == table && !before.contains_key(*key)).count()
}

/// After a write path: old rows keep their uuid, every row has a v4 uuid, and in every table from `tables` at least one
/// row has been added. Returns the new state.
async fn after_write(store: &Store, before: &Snapshot, step: &str, tables: &[&str]) -> Snapshot {
    let after = snapshot(store).await;
    assert_kept(before, &after, step);
    assert_all_v4(&after, step);
    for table in tables {
        assert!(added(before, &after, table) > 0, "{step}: keine neue Zeile in {table}");
    }
    after
}

/// After a deletion: exactly one new tombstone with kind and work uuid per row that disappeared, no other new
/// tombstones, all remaining rows unchanged. Returns the number of tombstones.
async fn after_delete(store: &Store, before: &Snapshot, graves_before: usize, step: &str) -> BTreeSet<&'static str> {
    let after = snapshot(store).await;
    let graves = tombstones(store).await;
    let mut kinds = BTreeSet::new();
    let mut removed = 0;
    for (key, row) in before {
        match after.get(key) {
            Some(now) => assert_eq!(now.uuid, row.uuid, "{step}: uuid von {key:?} hat sich geändert"),
            None => {
                removed += 1;
                let uuid = row.uuid.clone().unwrap();
                let grave = graves.get(&uuid).unwrap_or_else(|| panic!("{step}: kein Grabstein für {key:?}"));
                assert_eq!(grave.0, row.kind, "{step}: Art im Grabstein für {key:?}");
                assert_eq!(grave.1, row.work_uuid, "{step}: Werk-uuid im Grabstein für {key:?}");
                assert!(grave.1.is_some(), "{step}: Grabstein für {key:?} kennt sein Werk");
                kinds.insert(row.kind);
            }
        }
    }
    assert!(removed > 0, "{step}: nichts gelöscht");
    assert_eq!(graves.len(), graves_before + removed, "{step}: genau ein neuer Grabstein je gelöschter Zeile");
    kinds
}

fn new_work(title: &str, preset: Preset) -> NewWork<'_> {
    let kind = if preset == Preset::Sermon { WorkKind::Sermon } else { WorkKind::Book };
    NewWork { title, author: "Anna Apfel", kind, preset, subtitle: "", audience: "", bible: "" }
}

fn post(author: &str, body: &str, refs: Vec<TextRef>) -> Post {
    Post {
        id: 0,
        created: String::new(),
        author: author.into(),
        body: body.into(),
        sources: vec!["Notiz".into()],
        refs,
    }
}

fn svg(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

fn new_asset<'a>(file_name: &'a str, bytes: &'a [u8]) -> NewAsset<'a> {
    NewAsset { file_name, name: None, bytes, as_base64: false, path: None, caption: "Skizze", alt: "Skizze", note: "" }
}

fn mcp() -> IdeaOrigin {
    IdeaOrigin::Mcp { handle: "@mcp:probe".into() }
}

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn key(store: &Store, work: WorkId, seq: usize) -> i64 {
    store.load_work(work).await.unwrap().nodes[seq].key
}

#[tokio::test]
async fn every_write_path_gives_new_rows_a_canonical_v4_uuid() {
    let (_dir, store) = fresh().await;
    // Sample work: Store::open seeds it into the empty database (seed from baruch-core) – work, items, revisions.
    let empty = Snapshot::new();
    let snap = after_write(&store, &empty, "Beispielwerk", &["work", "node", "revision"]).await;

    let book = store.create_work(&new_work("Das Beet", Preset::Book), HUMAN).await.unwrap().work;
    let snap = after_write(&store, &snap, "create_work", &["work", "node", "revision"]).await;
    store
        .create_work_with_text(&new_work("Vom Säen", Preset::Sermon), HUMAN, Some("Ein Sämann ging aus."))
        .await
        .unwrap();
    let snap = after_write(&store, &snap, "create_work_with_text", &["work", "node", "revision"]).await;
    let orchard = store.import_typst(ORCHARD, "obstgarten.typ", HUMAN).await.unwrap().work;
    let snap =
        after_write(&store, &snap, "import_typst", &["work", "node", "revision", "work_template_revision"]).await;
    let barn =
        [ImportAsset { path: "bilder/scheune.svg".into(), file_name: "scheune.svg".into(), bytes: svg(BARN_SVG) }];
    store.import_typst_with_assets(BARN_SOURCE, "scheune.typ", HUMAN, "Import", None, &barn).await.unwrap();
    let snap =
        after_write(&store, &snap, "import_typst mit Bildern", &["work", "node", "asset", "asset_revision"]).await;

    store.outline_add(orchard, 2, Place::After, "Das Lager", HUMAN).await.unwrap();
    let snap = after_write(&store, &snap, "outline_add", &["node", "revision"]).await;
    let points =
        [NewPoint { title: "Der Most".into(), level: None }, NewPoint { title: "Das Mus".into(), level: None }];
    let human = Author::Human { name: HUMAN.into() };
    store.outline_add_many(orchard, Some(0), Place::Before, &points, &human).await.unwrap();
    let snap = after_write(&store, &snap, "outline_add_many", &["node", "revision"]).await;
    let point = NewPoint { title: "Die Bienen".into(), level: None };
    let outcome = store.outline_add_by_assistant(orchard, Some(0), Place::After, &[point], &mcp(), "probe", "").await;
    assert!(matches!(outcome, Ok(AssistantAdd::Added(_))), "Kapitel neben Kapitel legt sofort an: {outcome:?}");
    let snap = after_write(&store, &snap, "outline_add_by_assistant", &["node", "revision"]).await;

    let loaded = store.load_work(orchard).await.unwrap();
    let trees = loaded.nodes.iter().position(|n| n.title == "Die Bäume").expect("Kapitel „Die Bäume“");
    store.save_human(orchard, trees, "= Die Bäume\n\nApfel, Birne und Quitte.\n", HUMAN).await.unwrap().unwrap();
    let snap = after_write(&store, &snap, "save_human", &["revision"]).await;
    store.add_proposal(orchard, trees, "= Die Bäume\n\nApfel und Kirsche.\n", "@lektor", "probe", "").await.unwrap();
    let snap = after_write(&store, &snap, "add_proposal", &["revision"]).await;
    let refs = vec![TextRef { node: trees, rev: 1, quote: "Apfel".into() }];
    let post_id = store.add_post(orchard, trees, &post("@lektor", "Quitten brauchen Platz.", refs)).await.unwrap();
    let snap = after_write(&store, &snap, "add_post", &["post"]).await;
    let words = vec!["Quitte".to_owned()];
    let tree_key = key(&store, orchard, trees).await;
    let idea = NewIdea {
        work: Some(orchard),
        key: Some(tree_key),
        words: &words,
        note: "Quittengelee",
        quote: "",
        source: "",
        bible: "",
        origin: IdeaOrigin::Human { name: HUMAN.into() },
    };
    store.idea_create(&idea).await.unwrap();
    let snap = after_write(&store, &snap, "idea_create", &["idea"]).await;
    // Convert: new idea with a uuid of its own, the post goes (with a tombstone, it is deleted).
    let post_uuid = snap[&("post", post_id)].uuid.clone().unwrap();
    store.post_to_idea(orchard, post_id, &words).await.unwrap();
    let mut without_post = snap.clone();
    without_post.remove(&("post", post_id));
    let snap = after_write(&store, &without_post, "post_to_idea", &["idea"]).await;
    assert!(!snap.contains_key(&("post", post_id)), "post_to_idea: der Beitrag ist umgewandelt");
    let grave = tombstones(&store).await.remove(&post_uuid);
    assert_eq!(grave.map(|g| g.0), Some("post".to_owned()), "post_to_idea: Grabstein für den Beitrag");
    let (thread, _) = store.thread_create(orchard, HUMAN, "Wann ernten wir?").await.unwrap();
    let snap = after_write(&store, &snap, "thread_create", &["chat_thread", "chat_thread_post"]).await;
    store.thread_add_post(orchard, thread.id, "@lektor", "Im Oktober.", &[]).await.unwrap();
    let snap = after_write(&store, &snap, "thread_add_post", &["chat_thread_post"]).await;
    let first = svg("<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"2\"/></svg>\n");
    store.add_asset(book, &new_asset("apfel.svg", &first), HUMAN).await.unwrap();
    let snap = after_write(&store, &snap, "add_asset", &["asset", "asset_revision"]).await;
    let second = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"5\"/></svg>\n";
    store.replace_asset(book, "apfel.svg", "apfel.svg", second.as_bytes(), false, "größer", HUMAN).await.unwrap();
    let snap = after_write(&store, &snap, "replace_asset", &["asset_revision"]).await;
    let edit = WorkEdit { template: Some("#set text(lang: \"de\")\n"), note: Some("Sprache"), ..WorkEdit::default() };
    let update = store.update_work(book, &edit, HUMAN).await.unwrap();
    assert!(update.template_rev.is_some(), "neue Vorlage ergibt eine neue Fassung: {update:?}");
    after_write(&store, &snap, "update_work (neue Vorlagenfassung)", &["work_template_revision"]).await;
}

/// Work with subtree, several revisions, post, idea, thread and an image with two versions.
async fn orchard_with_everything(store: &Store) -> WorkId {
    let work = store.import_typst(ORCHARD, "obstgarten.typ", HUMAN).await.unwrap().work;
    store.save_human(work, 0, "= Die Bäume\n\nApfel, Birne, Pflaume.\n", HUMAN).await.unwrap().unwrap();
    store.add_proposal(work, 0, "= Die Bäume\n\nNur Äpfel.\n", "@lektor", "probe", "kürzer").await.unwrap();
    store.add_post(work, 1, &post("@lektor", "Schnitt im Winter?", vec![])).await.unwrap();
    let words = vec!["Leiter".to_owned()];
    let tree_key = key(store, work, 2).await;
    let idea = NewIdea {
        work: Some(work),
        key: Some(tree_key),
        words: &words,
        note: "Leitern prüfen",
        quote: "",
        source: "",
        bible: "",
        origin: mcp(),
    };
    store.idea_create(&idea).await.unwrap();
    let (thread, _) = store.thread_create(work, HUMAN, "Wer pflückt?").await.unwrap();
    store.thread_add_post(work, thread.id, "@lektor", "Alle.", &[]).await.unwrap();
    let first = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"2\"/></svg>\n";
    store.add_asset(work, &new_asset("korb.svg", first.as_bytes()), HUMAN).await.unwrap();
    let second = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"4\"/></svg>\n";
    store.replace_asset(work, "korb.svg", "korb.svg", second.as_bytes(), false, "voller", HUMAN).await.unwrap();
    let third = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"6\"/></svg>\n";
    store.replace_asset(work, "korb.svg", "korb.svg", third.as_bytes(), false, "übervoll", HUMAN).await.unwrap();
    work
}

/// After a change without deletion: every old row with the same uuid, no tombstone.
async fn assert_unchanged_ids(store: &Store, before: &Snapshot, step: &str) -> Snapshot {
    let after = snapshot(store).await;
    assert_kept(before, &after, step);
    assert_all_v4(&after, step);
    assert!(tombstones(store).await.is_empty(), "{step}: kein Grabstein, nichts gelöscht");
    after
}

#[tokio::test]
async fn moving_renaming_switching_and_editing_keep_every_uuid() {
    let (_dir, store) = fresh().await;
    let work = orchard_with_everything(&store).await;
    let snap = snapshot(&store).await;
    let harvest = key(&store, work, 2).await;
    let harvest_uuid = store.node_uuid(work, harvest).await.unwrap();

    // „Die Ernte“ (seq 2) before „Die Bäume“: the item keeps key and uuid.
    store.outline_move(work, 2, 0, Place::Before, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_move").await;
    assert_eq!(key(&store, work, 0).await, harvest, "der verschobene Punkt steht vorn");
    assert_eq!(store.node_uuid(work, harvest).await.unwrap(), harvest_uuid, "uuid des Punkts nach dem Verschieben");
    store.outline_rename(work, 0, "Die Lese", HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_rename").await;
    // „Der Schnitt“ (now seq 2, level 2) becomes a chapter and a section again.
    store.outline_set_level(work, 2, 1, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_set_level").await;
    store.outline_set_numbered(work, 2, false, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_set_numbered").await;

    // Revisions of „Die Bäume“ (now seq 1): 1 import, 2 saved (current), 3 proposal.
    store.switch_to(work, 1, 1, false, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "switch_to").await;
    store.accept(work, 1, 3).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "accept").await;
    let edit = RevisionEdit { body: None, note: Some("erste Fassung") };
    store.update_revision(work, 1, 1, &edit, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "update_revision").await;
    store.set_current_asset(work, "korb.svg", 1).await.unwrap();
    assert_unchanged_ids(&store, &snap, "set_current_asset").await;
}

#[tokio::test]
async fn deleting_leaves_one_tombstone_per_row_with_kind_and_work_uuid() {
    let (_dir, store) = fresh().await;
    let work = orchard_with_everything(&store).await;
    let work_uuid = store.work_uuid(work).await.unwrap();

    // Revision 3 (proposal) of „Die Bäume“.
    let snap = snapshot(&store).await;
    store.delete_revision(work, 0, 3, HUMAN).await.unwrap();
    let kinds = after_delete(&store, &snap, 0, "delete_revision").await;
    assert_eq!(kinds, BTreeSet::from(["revision"]), "nur die Fassung");

    let snap = snapshot(&store).await;
    let post_id = store.load_work(work).await.unwrap().nodes[1].posts[0].id;
    store.post_delete(work, post_id).await.unwrap();
    let kinds = after_delete(&store, &snap, 1, "post_delete").await;
    assert_eq!(kinds, BTreeSet::from(["post"]), "nur der Beitrag");

    let snap = snapshot(&store).await;
    store.delete_asset_revision(work, "korb.svg", 1, HUMAN).await.unwrap();
    let kinds = after_delete(&store, &snap, 2, "delete_asset_revision").await;
    assert_eq!(kinds, BTreeSet::from(["asset_revision"]), "nur die Bildfassung");

    let snap = snapshot(&store).await;
    store.delete_asset(work, "korb.svg", HUMAN).await.unwrap();
    let kinds = after_delete(&store, &snap, 3, "delete_asset").await;
    assert_eq!(kinds, BTreeSet::from(["asset", "asset_revision"]), "Bild samt übrigen Fassungen");

    // „Der Schnitt“ (seq 1, without sub-items) with revision and deleted post.
    let graves = tombstones(&store).await.len();
    let snap = snapshot(&store).await;
    store.add_post(work, 1, &post("@lektor", "Noch ein Gedanke.", vec![])).await.unwrap();
    let snap_with_post = snapshot(&store).await;
    assert_eq!(added(&snap, &snap_with_post, "post"), 1, "ein Beitrag am Abschnitt");
    store.outline_delete(work, 1, HUMAN).await.unwrap();
    let kinds = after_delete(&store, &snap_with_post, graves, "outline_delete").await;
    assert_eq!(kinds, BTreeSet::from(["node", "post", "revision"]), "Punkt mit Fassungen und Beiträgen");

    // Soft deletion of an idea: no tombstone, the row stays (restorable).
    let graves = tombstones(&store).await.len();
    let idea_id: i64 =
        sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    store.idea_delete(idea_id).await.unwrap();
    assert_eq!(tombstones(&store).await.len(), graves, "idea_delete ist weich: kein Grabstein");

    // Whole work: every row of the work (work, items, revisions, template revision, thread including posts) with the
    // work uuid; the idea stays without a work (ON DELETE SET NULL) and therefore has no tombstone.
    let snap = snapshot(&store).await;
    let title: String =
        sqlx::query_scalar("SELECT title FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    store.delete_work(work, &title).await.unwrap();
    let kinds = after_delete(&store, &snap, graves, "delete_work").await;
    let expected = BTreeSet::from(["work", "node", "revision", "template_revision", "thread", "thread_post"]);
    assert_eq!(kinds, expected, "alle Arten von Zeilen des Werks");
    let idea_row: Option<(Option<i64>, String)> = sqlx::query_as("SELECT work_id, uuid FROM idea WHERE id = ?")
        .bind(idea_id)
        .fetch_optional(store.pool())
        .await
        .unwrap();
    let (idea_work, idea_uuid) = idea_row.expect("der Einfall bleibt erhalten");
    assert_eq!(idea_work, None, "Einfall ohne Werk");
    assert!(!tombstones(&store).await.contains_key(&idea_uuid), "kein Grabstein für den Einfall");
    let graves = tombstones(&store).await;
    assert_eq!(graves.get(&work_uuid).map(|g| g.0.as_str()), Some("work"), "Grabstein des Werks");
    assert!(
        graves.values().all(|(_, w)| w.as_deref() == Some(work_uuid.as_str())),
        "alle Grabsteine dieses Tests gehören zum gelöschten Werk: {graves:?}"
    );
}

#[tokio::test]
async fn without_0042_threads_deleted_by_cascade_would_forget_their_work() {
    // Proves the reason for 0042: without the BEFORE triggers the cascade deletes thread and thread posts only when the
    // work is already gone; the tombstone trigger from 0041 then no longer finds a work uuid.
    let (_dir, store) = fresh().await;
    let work = store.create_work(&new_work("Der Zaun", Preset::Book), HUMAN).await.unwrap().work;
    let (thread, first) = store.thread_create(work, HUMAN, "Holz oder Draht?").await.unwrap();
    let _ = first;
    let thread_uuid: String = sqlx::query_scalar("SELECT uuid FROM chat_thread WHERE id = ?")
        .bind(thread.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    for trigger in ["work_cascade_uuid_tombstone", "chat_thread_cascade_uuid_tombstone"] {
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TRIGGER {trigger}"))).execute(store.pool()).await.unwrap();
    }
    store.delete_work(work, "Der Zaun").await.unwrap();
    let graves = tombstones(&store).await;
    assert_eq!(graves.get(&thread_uuid), Some(&("thread".to_owned(), None)), "ohne 0042: Grabstein ohne Werk");
    assert!(
        graves.values().filter(|(k, _)| k == "thread_post").all(|(_, w)| w.is_none()),
        "ohne 0042: auch die Fadenbeiträge ohne Werk: {graves:?}"
    );
}

#[tokio::test]
async fn a_tombstone_stops_nothing_in_the_store_itself() {
    let (_dir, store) = fresh().await;
    let work = store.create_work(&new_work("Die Hecke", Preset::Book), HUMAN).await.unwrap().work;
    let id = store.add_post(work, 0, &post(HUMAN, "Liguster.", vec![])).await.unwrap();
    let gone: String =
        sqlx::query_scalar("SELECT uuid FROM post WHERE id = ?").bind(id).fetch_one(store.pool()).await.unwrap();
    store.post_delete(work, id).await.unwrap();
    assert!(tombstones(&store).await.contains_key(&gone), "Grabstein für den Beitrag");
    // The store does not look into the tombstones: a new post is created as always, and a row may carry the buried uuid
    // again (this is how `exchange_apply` creates on import as a new work, contract F4).
    let again = store.add_post(work, 0, &post(HUMAN, "Hainbuche.", vec![])).await.unwrap();
    sqlx::query("UPDATE post SET uuid = ? WHERE id = ?").bind(&gone).bind(again).execute(store.pool()).await.unwrap();
    // A second deletion of the same uuid is not an error (INSERT OR IGNORE); the first tombstone stays.
    let first_time = tombstones(&store).await[&gone].clone();
    store.post_delete(work, again).await.expect("zweites Löschen derselben uuid geht");
    assert_eq!(tombstones(&store).await[&gone], first_time, "der erste Grabstein bleibt, keiner doppelt");
}

#[tokio::test]
async fn uuid_lookups_answer_only_for_canonical_uuids_of_the_right_work() {
    let (_dir, store) = fresh().await;
    let a = store.create_work(&new_work("Der Teich", Preset::Book), HUMAN).await.unwrap().work;
    let b = store.create_work(&new_work("Der Bach", Preset::Book), HUMAN).await.unwrap().work;
    let a_uuid = store.work_uuid(a).await.unwrap();
    let b_uuid = store.work_uuid(b).await.unwrap();
    assert!(is_v4(&a_uuid) && is_v4(&b_uuid), "Werk-uuids kanonisch v4: {a_uuid} {b_uuid}");
    assert_ne!(a_uuid, b_uuid, "zwei Werke, zwei uuids");
    assert_eq!(store.work_by_uuid(&a_uuid).await.unwrap(), Some(a), "Werk über seine uuid");
    assert_eq!(store.work_meta(a).await.unwrap().uuid, a_uuid, "work_meta trägt dieselbe uuid");
    assert!(matches!(store.work_uuid(9999).await, Err(StoreError::UnknownWork(9999))), "unbekanntes Werk");
    let unknown = baruch_store::new_uuid();
    assert_eq!(store.work_by_uuid(&unknown).await.unwrap(), None, "unbekannte uuid → None");
    // Not canonical (upper case, without hyphens, with whitespace): no hit, the column holds only lower case.
    for odd in [a_uuid.to_uppercase(), a_uuid.replace('-', ""), format!(" {a_uuid}"), String::new()] {
        assert_eq!(store.work_by_uuid(&odd).await.unwrap(), None, "nicht kanonisch „{odd}“ → None");
    }

    let a_key = key(&store, a, 0).await;
    let b_key = key(&store, b, 0).await;
    let a_node = store.node_uuid(a, a_key).await.unwrap();
    assert!(is_v4(&a_node), "Punkt-uuid kanonisch: {a_node}");
    assert_eq!(store.node_key_by_uuid(a, &a_node).await.unwrap(), Some(a_key), "Punkt über seine uuid");
    assert_eq!(store.node_key_by_uuid(b, &a_node).await.unwrap(), None, "fremdes Werk: kein Treffer");
    assert_eq!(store.node_key_by_uuid(a, &a_node.to_uppercase()).await.unwrap(), None, "nicht kanonisch → None");
    assert_eq!(store.node_key_by_uuid(a, &unknown).await.unwrap(), None, "unbekannte uuid → None");
    assert_eq!(store.node_key_by_uuid(a, &a_uuid).await.unwrap(), None, "Werk-uuid ist kein Punkt");
    assert!(
        matches!(store.node_uuid(a, b_key).await, Err(StoreError::GoneNode { work, key }) if work == a && key == b_key),
        "Schlüssel aus fremdem Werk → GoneNode"
    );
    assert!(matches!(store.node_uuid(9999, a_key).await, Err(StoreError::GoneNode { .. })), "unbekanntes Werk");
}

#[tokio::test]
async fn the_instance_has_a_stable_uuid_and_a_checked_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let first = store.instance().await.unwrap();
    assert!(is_v4(&first.uuid), "Instanzkennung kanonisch v4: {}", first.uuid);
    assert_eq!(first.name, "", "Name anfangs leer");
    assert_eq!(first.label(), format!("ohne Namen ({})", &first.uuid[..8]), "Anzeige ohne Namen");

    let named = store.set_instance_name("  Arbeitszimmer \t").await.unwrap();
    assert_eq!((named.uuid.as_str(), named.name.as_str()), (first.uuid.as_str(), "Arbeitszimmer"), "getrimmt");
    assert_eq!(named.label(), "Arbeitszimmer", "Anzeige mit Namen");
    // Maximum length in characters, not in bytes: 80 umlauts (160 bytes) work, 81 do not.
    let longest = "ä".repeat(MAX_INSTANCE_NAME_CHARS);
    assert_eq!(store.set_instance_name(&longest).await.unwrap().name, longest, "80 Zeichen gehen");
    let too_long = store.set_instance_name(&"ä".repeat(MAX_INSTANCE_NAME_CHARS + 1)).await;
    assert!(
        matches!(&too_long, Err(StoreError::Exchange(m)) if m.contains("höchstens 80 Zeichen")),
        "81 Zeichen abgelehnt: {too_long:?}"
    );
    for bad in ["Erste\nZeile", "Tab\tim Namen", "Glocke\u{7}", "Löschen\u{7f}", "C1\u{85}Zeichen"] {
        let refused = store.set_instance_name(bad).await;
        assert!(
            matches!(&refused, Err(StoreError::Exchange(m)) if m.contains("Steuerzeichen")),
            "Steuerzeichen in {bad:?} abgelehnt: {refused:?}"
        );
    }
    assert_eq!(store.instance().await.unwrap().name, longest, "abgelehnte Namen ändern nichts");
    let reset = store.set_instance_name("   ").await.unwrap();
    assert_eq!(reset.name, "", "nur Leerraum = zurücksetzen");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM store_meta WHERE key = 'instance_name'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "zurücksetzen entfernt die Zeile instance_name");
    store.set_instance_name("Laptop").await.unwrap();
    drop(store);

    let again = Store::open(dir.path()).await.unwrap();
    let reopened = again.instance().await.unwrap();
    assert_eq!(reopened.uuid, first.uuid, "Instanzkennung bleibt über ein erneutes Öffnen gleich");
    assert_eq!(reopened.name, "Laptop", "Name bleibt gespeichert");
}

/// Round 25: the tables of the idea map follow the same rules as the ten from 0041. The rows are created here through
/// SQL without a uuid (like every write path of the store, which never names the column); the write paths themselves
/// belong to karte-daten.
#[tokio::test]
async fn links_and_positions_get_v4_uuids_and_leave_tombstones_with_the_work_uuid() {
    let (_dir, store) = fresh().await;
    let work = store.import_typst(ORCHARD, "obstgarten.typ", HUMAN).await.unwrap().work;
    let work_uuid = store.work_uuid(work).await.unwrap();
    let mut ideas = Vec::new();
    for word in ["Apfel", "Birne", "Leiter"] {
        let words = vec![word.to_owned()];
        let idea = NewIdea {
            work: Some(work),
            key: None,
            words: &words,
            note: "",
            quote: "",
            source: "",
            bible: "",
            origin: mcp(),
        };
        ideas.push(store.idea_create(&idea).await.unwrap().id);
    }
    let pool = store.pool();
    for (from, to) in [(ideas[0], ideas[1]), (ideas[1], ideas[2])] {
        sqlx::query("INSERT INTO idea_link (from_id, to_id, kind, origin_kind, origin_name) VALUES (?, ?, 'leads_to', 'mcp', '@mcp:probe')")
            .bind(from)
            .bind(to)
            .execute(pool)
            .await
            .unwrap();
    }
    for idea in &ideas {
        sqlx::query("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES (?, ?, 1.5, -2.5)")
            .bind(work)
            .bind(idea)
            .execute(pool)
            .await
            .unwrap();
    }
    let links: Vec<String> = sqlx::query_scalar("SELECT uuid FROM idea_link").fetch_all(pool).await.unwrap();
    let positions: Vec<String> = sqlx::query_scalar("SELECT uuid FROM idea_map_pos").fetch_all(pool).await.unwrap();
    assert_eq!((links.len(), positions.len()), (2, 3), "Vorbedingung: zwei Verbindungen, drei Lagen");
    for uuid in links.iter().chain(&positions) {
        assert!(is_v4(uuid), "Fülltrigger aus 0043 vergibt eine kanonische v4-uuid: {uuid}");
    }
    let graves = tombstones(&store).await;
    // Idea 2 hard-deleted (only conceivable in a repair): it is the target of the one and the source of the other link;
    // both and its position go. The BEFORE DELETE trigger `idea_link_cascade_uuid_tombstone` from 0043 deletes the
    // links while the idea row still exists – otherwise the tombstone of the link whose `from_id` it is would write
    // work_uuid NULL.
    sqlx::query("DELETE FROM idea WHERE id = ?").bind(ideas[1]).execute(pool).await.unwrap();
    let after = tombstones(&store).await;
    let new: Vec<(String, Option<String>)> =
        after.iter().filter(|(u, _)| !graves.contains_key(*u)).map(|(_, (k, w))| (k.clone(), w.clone())).collect();
    let count = |kind: &str| new.iter().filter(|(k, _)| k == kind).count();
    assert_eq!(
        count("idea_link"),
        2,
        "beide Verbindungen (als Quelle und als Ziel) hinterlassen einen Grabstein: {new:?}"
    );
    assert_eq!(count("idea_map_pos"), 1, "seine Lage hinterlässt einen Grabstein: {new:?}");
    assert!(new.iter().all(|(_, w)| w.as_deref() == Some(work_uuid.as_str())), "alle mit der Werk-uuid: {new:?}");
    // Whole work deleted: the remaining positions go before the work (trigger as in 0042) and know its uuid.
    let title = store.works().await.unwrap().into_iter().find(|w| w.0 == work).unwrap().2;
    store.delete_work(work, &title).await.unwrap();
    let last = tombstones(&store).await;
    for uuid in &positions {
        assert_eq!(
            last.get(uuid).map(|(k, w)| (k.as_str(), w.as_deref())),
            Some(("idea_map_pos", Some(work_uuid.as_str()))),
            "Lage {uuid}: Grabstein mit Werk-uuid, auch über das gelöschte Werk"
        );
    }
}

/// All `.rs` files below `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_code_path_but_exchange_apply_writes_the_uuid_column() {
    // Contract 0041: new rows get their uuid only through the fill triggers; only `exchange_apply` may set it (on rows
    // just created). Checked is the source of all crates: no line outside of exchange_apply.rs writes `SET uuid` or
    // names uuid in an `INSERT INTO`.
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap() {
        let src = entry.unwrap().path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    assert!(files.len() > 50, "Quelltext gefunden: {} Dateien", files.len());
    let mut offenders = Vec::new();
    for file in &files {
        if file.file_name().is_some_and(|n| n == "exchange_apply.rs") {
            continue;
        }
        let text = std::fs::read_to_string(file).unwrap();
        for (n, line) in text.lines().enumerate() {
            let upper = line.to_uppercase();
            // SQL, not Rust: `INSERT INTO` (not `HashSet::insert`) and `, uuid =` without `>` (not `uuid => …` in
            // minijinja contexts); both came as a false alarm with export and import.
            let writes = upper.contains("SET UUID")
                || (upper.contains("INSERT INTO") && line.contains("uuid") && !line.contains("instance_uuid"))
                || upper.match_indices(", UUID =").any(|(at, m)| !upper[at + m.len()..].starts_with('>'));
            if writes {
                offenders.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(offenders.is_empty(), "uuid wird außerhalb von exchange_apply geschrieben:\n{}", offenders.join("\n"));
}
