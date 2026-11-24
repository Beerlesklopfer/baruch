// SPDX-License-Identifier: AGPL-3.0-or-later
//! Weltweit eindeutige Kennungen (Runde 26 „ids“, Migrationen 0041 und 0042) auf einer echten SQLite-Datei im
//! Temp-Ordner, Testtexte frei erfunden.
//!
//! Belegt:
//! - Jeder Schreibweg des Stores vergibt neuen Zeilen eine kanonische v4-uuid (über die Fülltrigger), ohne die
//!   Spalte selbst zu nennen.
//! - Verschieben, Umbenennen, Ebene, Nummer, Umschalten, Übernehmen, Ändern einer Fassung und Umschalten eines
//!   Bilds erhalten jede vorhandene uuid.
//! - Löschen hinterlässt je gelöschter Zeile genau einen Grabstein mit Art und Werk-uuid, auch für Fäden, die mit
//!   dem Werk per Kaskade gehen (0042); weiches Löschen eines Einfalls hinterlässt keinen; ein Grabstein hält
//!   im Store nichts auf.
//! - Die Abfragen über uuid (`work_uuid`, `work_by_uuid`, `node_key_by_uuid`, `node_uuid`) und der Name der
//!   Instanz.
//! - Kein Codepfad außer `exchange_apply` schreibt die Spalte uuid (Quelltextprüfung).
//! - Runde 25 (Migration 0043): Verbindungen (`idea_link`) und Lagen (`idea_map_pos`) bekommen ihre uuid ebenso
//!   über Fülltrigger und hinterlassen Grabsteine `idea_link`/`idea_map_pos` mit der Werk-uuid, auch über die
//!   Kaskade eines hart gelöschten Einfalls bzw. Werks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use baruch_core::{Author, Post, TextRef, WorkKind};
use baruch_store::{
    AssistantAdd, IdeaOrigin, ImportAsset, MAX_INSTANCE_NAME_CHARS, NewAsset, NewIdea, NewPoint, NewWork, Place,
    Preset, RevisionEdit, Store, StoreError, WorkEdit, WorkId,
};

const HUMAN: &str = "Anna";

/// Frei erfundenes Buch mit Werkvorlage, zwei Kapiteln und einem Abschnitt.
const ORCHARD: &str = "#set document(title: \"Der Obstgarten\", author: \"Anna Apfel\")\n\n\
= Die Bäume\n\nApfel und Birne.\n\n== Der Schnitt\n\nIm Februar.\n\n= Die Ernte\n\nKörbe und Leitern.\n";

/// Frei erfundenes Buch mit einem Bild.
const BARN_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"4\" height=\"3\"/></svg>\n";
const BARN_SOURCE: &str = "#set document(title: \"Die Scheune\")\n\n= Das Dach\n\n#image(\"bilder/scheune.svg\")\n";

/// Die zehn Tabellen mit uuid und ihre Art im Grabstein.
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

/// Je Tabelle: id, uuid und die uuid des Werks, zu dem die Zeile gehört (so, wie der Grabstein-Trigger sie
/// bestimmt).
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

/// Eine Zeile mit uuid: Art (für den Grabstein), uuid, Werk-uuid.
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

/// Kanonisch klein 8-4-4-4-12, Version 4, Variante 10 (RFC 9562).
fn is_v4(uuid: &str) -> bool {
    baruch_exchange::is_uuid(uuid) && &uuid[14..15] == "4" && matches!(&uuid[19..20], "8" | "9" | "a" | "b")
}

/// Jede Zeile jeder Tabelle hat eine kanonische v4-uuid, und keine kommt doppelt vor (auch nicht über
/// Tabellen hinweg).
fn assert_all_v4(snap: &Snapshot, step: &str) {
    let mut seen = BTreeSet::new();
    for ((table, id), row) in snap {
        let uuid = row.uuid.as_deref().unwrap_or_else(|| panic!("{step}: {table} {id} ohne uuid"));
        assert!(is_v4(uuid), "{step}: {table} {id} hat keine kanonische v4-uuid: {uuid}");
        assert!(seen.insert(uuid.to_owned()), "{step}: uuid {uuid} doppelt ({table} {id})");
    }
}

/// Alle Zeilen von `before` stehen in `after` mit derselben uuid (nichts gelöscht, nichts umgeschrieben).
fn assert_kept(before: &Snapshot, after: &Snapshot, step: &str) {
    for (key, row) in before {
        let now = after.get(key).unwrap_or_else(|| panic!("{step}: Zeile {key:?} verschwunden"));
        assert_eq!(now.uuid, row.uuid, "{step}: uuid von {key:?} hat sich geändert");
    }
}

/// Neue Zeilen in `table` zwischen zwei Ständen.
fn added(before: &Snapshot, after: &Snapshot, table: &str) -> usize {
    after.keys().filter(|key| key.0 == table && !before.contains_key(*key)).count()
}

/// Nach einem Schreibweg: Alte Zeilen behalten ihre uuid, jede Zeile hat eine v4-uuid, und in jeder Tabelle
/// aus `tables` ist mindestens eine Zeile dazugekommen. Gibt den neuen Stand zurück.
async fn after_write(store: &Store, before: &Snapshot, step: &str, tables: &[&str]) -> Snapshot {
    let after = snapshot(store).await;
    assert_kept(before, &after, step);
    assert_all_v4(&after, step);
    for table in tables {
        assert!(added(before, &after, table) > 0, "{step}: keine neue Zeile in {table}");
    }
    after
}

/// Nach einem Löschen: Je verschwundener Zeile genau ein neuer Grabstein mit Art und Werk-uuid, keine
/// anderen neuen Grabsteine, alle übrigen Zeilen unverändert. Gibt die Zahl der Grabsteine zurück.
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
    // Beispielwerk: Store::open sät es in die leere Datenbank (Saat aus baruch-core) – Werk, Punkte, Fassungen.
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
    // Umwandeln: neuer Einfall mit eigener uuid, der Beitrag geht (mit Grabstein, er ist gelöscht).
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

/// Werk mit Teilbaum, mehreren Fassungen, Beitrag, Einfall, Faden und Bild mit zwei Fassungen.
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

/// Nach einer Änderung ohne Löschen: jede alte Zeile mit derselben uuid, kein Grabstein.
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

    // „Die Ernte“ (seq 2) vor „Die Bäume“: der Punkt behält key und uuid.
    store.outline_move(work, 2, 0, Place::Before, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_move").await;
    assert_eq!(key(&store, work, 0).await, harvest, "der verschobene Punkt steht vorn");
    assert_eq!(store.node_uuid(work, harvest).await.unwrap(), harvest_uuid, "uuid des Punkts nach dem Verschieben");
    store.outline_rename(work, 0, "Die Lese", HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_rename").await;
    // „Der Schnitt“ (jetzt seq 2, Ebene 2) wird Kapitel und wieder Abschnitt.
    store.outline_set_level(work, 2, 1, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_set_level").await;
    store.outline_set_numbered(work, 2, false, HUMAN).await.unwrap();
    let snap = assert_unchanged_ids(&store, &snap, "outline_set_numbered").await;

    // Fassungen von „Die Bäume“ (jetzt seq 1): 1 Import, 2 gespeichert (aktuell), 3 Vorschlag.
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

    // Fassung 3 (Vorschlag) von „Die Bäume“.
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

    // „Der Schnitt“ (seq 1, ohne Unterpunkte) mit Fassung und gelöschtem Beitrag.
    let graves = tombstones(&store).await.len();
    let snap = snapshot(&store).await;
    store.add_post(work, 1, &post("@lektor", "Noch ein Gedanke.", vec![])).await.unwrap();
    let snap_with_post = snapshot(&store).await;
    assert_eq!(added(&snap, &snap_with_post, "post"), 1, "ein Beitrag am Abschnitt");
    store.outline_delete(work, 1, HUMAN).await.unwrap();
    let kinds = after_delete(&store, &snap_with_post, graves, "outline_delete").await;
    assert_eq!(kinds, BTreeSet::from(["node", "post", "revision"]), "Punkt mit Fassungen und Beiträgen");

    // Weiches Löschen eines Einfalls: kein Grabstein, die Zeile bleibt (wiederherstellbar).
    let graves = tombstones(&store).await.len();
    let idea_id: i64 =
        sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ?").bind(work).fetch_one(store.pool()).await.unwrap();
    store.idea_delete(idea_id).await.unwrap();
    assert_eq!(tombstones(&store).await.len(), graves, "idea_delete ist weich: kein Grabstein");

    // Ganzes Werk: jede Zeile des Werks (Werk, Punkte, Fassungen, Vorlagenfassung, Faden samt Beiträgen) mit
    // Werk-uuid; der Einfall bleibt ohne Werk (ON DELETE SET NULL) und hat darum keinen Grabstein.
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
    // Belegt den Grund für 0042: Ohne die BEFORE-Trigger löscht die Kaskade Faden und Fadenbeiträge erst,
    // wenn das Werk schon fort ist; der Grabstein-Trigger aus 0041 findet dann keine Werk-uuid mehr.
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
        sqlx::query(&format!("DROP TRIGGER {trigger}")).execute(store.pool()).await.unwrap();
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
    // Der Store schaut nicht in die Grabsteine: Ein neuer Beitrag entsteht wie immer, und eine Zeile darf die
    // begrabene uuid wieder tragen (so legt `exchange_apply` beim Einlesen als neues Werk an, Vertrag F4).
    let again = store.add_post(work, 0, &post(HUMAN, "Hainbuche.", vec![])).await.unwrap();
    sqlx::query("UPDATE post SET uuid = ? WHERE id = ?").bind(&gone).bind(again).execute(store.pool()).await.unwrap();
    // Ein zweites Löschen derselben uuid ist kein Fehler (INSERT OR IGNORE); der erste Grabstein bleibt.
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
    // Nicht kanonisch (groß, ohne Bindestriche, mit Leerraum): kein Treffer, die Spalte hält nur Kleinschreibung.
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
    // Höchstlänge in Zeichen, nicht in Bytes: 80 Umlaute (160 Bytes) gehen, 81 nicht.
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

/// Runde 25: Die Tabellen der Gedankenkarte folgen denselben Regeln wie die zehn aus 0041. Die Zeilen entstehen hier
/// über SQL ohne uuid (wie jeder Schreibweg des Stores, der die Spalte nie nennt); die Schreibwege selbst gehören
/// karte-daten.
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
    // Einfall 2 hart gelöscht (nur in einer Reparatur denkbar): Er ist Ziel der einen und Quelle der anderen
    // Verbindung; beide und seine Lage gehen. Der BEFORE-DELETE-Trigger `idea_link_cascade_uuid_tombstone` aus 0043
    // löscht die Verbindungen, solange die Einfallzeile noch steht – sonst schriebe der Grabstein der Verbindung, deren
    // `from_id` er ist, work_uuid NULL.
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
    // Ganzes Werk gelöscht: die übrigen Lagen gehen vor dem Werk (Trigger wie 0042) und kennen seine uuid.
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

/// Alle `.rs`-Dateien unter `dir`, rekursiv.
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
    // Vertrag 0041: Neue Zeilen bekommen ihre uuid nur über die Fülltrigger; setzen darf sie nur
    // `exchange_apply` (auf eben angelegten Zeilen). Geprüft wird der Quelltext aller Crates: Keine Zeile außerhalb
    // von exchange_apply.rs schreibt `SET uuid` oder nennt uuid in einem `INSERT INTO`.
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
            // SQL, nicht Rust: `INSERT INTO` (nicht `HashSet::insert`) und `, uuid =` ohne `>` (nicht `uuid => …`
            // in minijinja-Kontexten); beides kam mit export und import als Fehlalarm.
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
