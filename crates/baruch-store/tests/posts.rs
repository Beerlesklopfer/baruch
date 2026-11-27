// SPDX-License-Identifier: AGPL-3.0-or-later
//! Managing posts (contract Round 8, 3b): read, delete, move and convert into an idea, each in one transaction. Real
//! SQLite file in the temp directory, test texts freely invented.

use baruch_core::{Post, TextRef};
use baruch_store::{IdeaFilter, IdeaOrigin, IdeaScope, Store, StoreError, WorkId};

const HUMAN: &str = "Anna";

const BEETE: &str = "\
= Die Beete

Kohl und Mangold.

== Kräuter

Thymian am Rand.

= Der Zaun

Haselruten.
";

async fn garden() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(BEETE, "beete.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

fn post(author: &str, body: &str) -> Post {
    Post { id: 0, created: String::new(), author: author.into(), body: body.into(), sources: vec![], refs: vec![] }
}

async fn posts_in(store: &Store, work: WorkId, node: usize) -> Vec<(i64, String)> {
    let loaded = store.load_work(work).await.unwrap();
    loaded.nodes[node].posts.iter().map(|p| (p.id, p.body.clone())).collect()
}

#[tokio::test]
async fn a_post_is_read_by_its_id_within_its_work_only() {
    let (_dir, store, work) = garden().await;
    let mut with_refs = post("@lektor", "Thymian blüht lila.");
    with_refs.sources = vec!["Kräuterbuch, S. 3".into()];
    with_refs.refs = vec![TextRef { node: 0, rev: 1, quote: "Kohl".into() }];
    let id = store.add_post(work, 1, &with_refs).await.unwrap();
    let info = store.post_get(work, id).await.unwrap();
    assert_eq!((info.id, info.work, info.node), (id, work, 1), "id, Werk und Abschnitt (seq)");
    let key = store.load_work(work).await.unwrap().nodes[1].key;
    assert_eq!(info.key, key, "fester Schlüssel des Abschnitts");
    assert_eq!((info.author.as_str(), info.body.as_str()), ("@lektor", "Thymian blüht lila."), "Autor und Text");
    assert_eq!(info.sources, with_refs.sources, "Quellen");
    assert_eq!(info.refs, with_refs.refs, "Verweise");
    assert!(!info.created.is_empty(), "Zeitpunkt aus der Datenbank");

    let other = store.default_work().await.unwrap();
    assert_ne!(other, work, "das Beispielwerk ist ein anderes Werk");
    let err = store.post_get(other, id).await.unwrap_err();
    assert!(
        matches!(err, StoreError::UnknownPost { work: w, post } if w == other && post == id),
        "eine id aus einem anderen Werk gilt dort nicht: {err:?}"
    );
    assert_eq!(err.to_string(), format!("Beitrag {id} gibt es in Werk {other} nicht."), "deutsche Meldung");
    assert!(matches!(store.post_get(999, id).await, Err(StoreError::UnknownWork(999))), "unbekanntes Werk");
}

#[tokio::test]
async fn deleting_removes_the_post_and_its_refs() {
    let (_dir, store, work) = garden().await;
    let keep = store.add_post(work, 0, &post("Anna", "Mangold säen.")).await.unwrap();
    let mut gone = post("@lektor", "Kohlweißling beachten.");
    gone.refs = vec![TextRef { node: 0, rev: 1, quote: "Kohl".into() }];
    let id = store.add_post(work, 0, &gone).await.unwrap();

    let deleted = store.post_delete(work, id).await.unwrap();
    assert_eq!(deleted.body, "Kohlweißling beachten.", "der gelöschte Beitrag kommt zurück (für den Commit)");
    assert!(deleted.entry_markdown().contains("Kohlweißling beachten."), "Eintrag mit dem ganzen Text");
    assert_eq!(posts_in(&store, work, 0).await, vec![(keep, "Mangold säen.".to_owned())], "nur der andere bleibt");
    let refs: i64 = sqlx::query_scalar("SELECT count(*) FROM post_ref WHERE post_id = ?")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(refs, 0, "seine Verweise sind mit fort");
    assert!(
        matches!(store.post_delete(work, id).await, Err(StoreError::UnknownPost { .. })),
        "zweites Löschen: den Beitrag gibt es nicht mehr"
    );
}

#[tokio::test]
async fn moving_appends_to_the_target_and_refuses_the_same_section() {
    let (_dir, store, work) = garden().await;
    let first = store.add_post(work, 2, &post("Anna", "Ruten schneiden.")).await.unwrap();
    let id = store.add_post(work, 0, &post("Anna", "Gehört zum Zaun.")).await.unwrap();
    let moved = store.post_move(work, id, 2).await.unwrap();
    assert_eq!(moved.node, 2, "jetzt im Zaun");
    let zaun = store.load_work(work).await.unwrap().nodes[2].clone();
    assert_eq!(moved.key, zaun.key, "Schlüssel des Ziels");
    let ids: Vec<i64> = zaun.posts.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![first, id], "ans Ende des Ziels (position = größte + 1)");
    assert!(posts_in(&store, work, 0).await.is_empty(), "aus den Beeten fort");

    let same = store.post_move(work, id, 2).await;
    assert!(matches!(same, Err(StoreError::Refused(_))), "gleiches Ziel abgelehnt: {same:?}");
    let unknown = store.post_move(work, id, 99).await;
    assert!(matches!(unknown, Err(StoreError::UnknownNode { node: 99, .. })), "unbekanntes Ziel: {unknown:?}");
    let other = store.default_work().await.unwrap();
    assert!(
        matches!(store.post_move(other, id, 0).await, Err(StoreError::UnknownPost { .. })),
        "nur innerhalb des eigenen Werks"
    );
    assert_eq!(store.post_get(work, id).await.unwrap().node, 2, "nach den Ablehnungen unverändert im Zaun");
}

#[tokio::test]
async fn converting_creates_the_idea_and_deletes_the_post_in_one_go() {
    let (_dir, store, work) = garden().await;
    let mut lektor = post("@lektor", "Thymian zwischen die Platten?");
    lektor.sources = vec!["Gartenbuch".into(), "Nachbarin".into()];
    let id = store.add_post(work, 1, &lektor).await.unwrap();
    let key = store.load_work(work).await.unwrap().nodes[1].key;
    let words = vec!["Thymian".to_owned(), "Wege".to_owned()];
    let (info, idea) = store.post_to_idea(work, id, &words).await.unwrap();
    assert_eq!(info.id, id, "der umgewandelte Beitrag kommt mit zurück");
    assert_eq!((idea.work, idea.key), (Some(work), Some(key)), "Einfall mit Werk und Bezug auf den Abschnitt");
    assert_eq!(idea.note, "Thymian zwischen die Platten?", "Gedanke = Text des Beitrags");
    assert_eq!(idea.source, "Gartenbuch; Nachbarin", "Quellen mit „; “");
    assert_eq!(idea.words, words, "Schlagwörter");
    assert_eq!(idea.origin, IdeaOrigin::Persona { handle: "@lektor".into() }, "Persona-Handle → Persona");
    assert!(posts_in(&store, work, 1).await.is_empty(), "der Beitrag ist fort");

    let mcp = store.add_post(work, 1, &post("@mcp:probe", "Wege mulchen.")).await.unwrap();
    let (_, idea) = store.post_to_idea(work, mcp, &words).await.unwrap();
    assert_eq!(idea.origin, IdeaOrigin::Mcp { handle: "@mcp:probe".into() }, "@mcp: → MCP");
    let human = store.add_post(work, 1, &post("Anna", "Steine sammeln.")).await.unwrap();
    let (_, idea) = store.post_to_idea(work, human, &words).await.unwrap();
    assert_eq!(idea.origin, IdeaOrigin::Human { name: "Anna".into() }, "sonst der Mensch");
    let stranger = store.add_post(work, 1, &post("@niemand", "Kein Persona-Eintrag.")).await.unwrap();
    let (_, idea) = store.post_to_idea(work, stranger, &words).await.unwrap();
    assert_eq!(idea.origin, IdeaOrigin::Human { name: "@niemand".into() }, "unbekannter Handle ist keine Persona");
}

#[tokio::test]
async fn a_failing_idea_keeps_the_post_and_creates_nothing() {
    let (_dir, store, work) = garden().await;
    let id = store.add_post(work, 0, &post("Anna", "Kohl häufeln.")).await.unwrap();
    let ideas_before = store.idea_list(&IdeaFilter { scope: IdeaScope::All, ..IdeaFilter::default() }).await.unwrap();
    let empty = store.post_to_idea(work, id, &["  ".to_owned()]).await;
    assert!(matches!(empty, Err(StoreError::Idea(_))), "ohne Schlagwort scheitert der Einfall: {empty:?}");
    let long = vec!["x".repeat(61)];
    assert!(matches!(store.post_to_idea(work, id, &long).await, Err(StoreError::Idea(_))), "zu langes Schlagwort");
    assert_eq!(posts_in(&store, work, 0).await, vec![(id, "Kohl häufeln.".to_owned())], "der Beitrag bleibt");
    let ideas_after = store.idea_list(&IdeaFilter { scope: IdeaScope::All, ..IdeaFilter::default() }).await.unwrap();
    assert_eq!(ideas_after.len(), ideas_before.len(), "kein halber Einfall (eine Transaktion)");
    let long_source = vec!["Quelle ".repeat(40)];
    let mut sourced = post("Anna", "Mit langer Quelle.");
    sourced.sources = long_source;
    let id = store.add_post(work, 0, &sourced).await.unwrap();
    let (_, idea) = store.post_to_idea(work, id, &["Quelle".to_owned()]).await.unwrap();
    assert!(idea.source.chars().count() <= 200, "die Quelle wird auf 200 Zeichen gekürzt statt abgelehnt");
}

#[tokio::test]
async fn node_key_names_the_fixed_key_of_a_position() {
    let (_dir, store, work) = garden().await;
    let loaded = store.load_work(work).await.unwrap();
    for node in &loaded.nodes {
        assert_eq!(store.node_key(work, node.id).await.unwrap(), node.key, "Stelle {} → Schlüssel", node.id);
    }
    assert!(matches!(store.node_key(work, 42).await, Err(StoreError::UnknownNode { node: 42, .. })), "unbekannt");
}
