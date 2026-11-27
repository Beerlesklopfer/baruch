// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“ (store-5): `Store::outline_delete_losses` nennt, was das Löschen eines Gliederungspunkts außer
//! der aktuellen Fassung mitlöscht. Echte SQLite-Datei im Temp-Ordner, Testtexte frei erfunden.

use baruch_core::Post;
use baruch_store::{LostPost, LostRevision, OutlineLosses, Store, StoreError, WorkId};

const HUMAN: &str = "Jorid";

const ORCHARD: &str = "\
= Die Obstwiese

Drei Reihen Apfelbäume.

= Der Schuppen

Leitern und Körbe.

= Die Presse

Sie quietscht.
";

async fn orchard() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(ORCHARD, "obstwiese.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

fn post(author: &str, body: &str) -> Post {
    Post { id: 0, created: String::new(), author: author.into(), body: body.into(), sources: vec![], refs: vec![] }
}

/// Zeilenzahl jeder Tabelle: der ganze Stand der Datenbank in Zahlen.
async fn row_counts(store: &Store) -> Vec<(String, i64)> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(store.pool())
        .await
        .unwrap();
    let mut counts = Vec::new();
    for table in tables {
        let n: i64 = match table.as_str() {
            "revision" => sqlx::query_scalar("SELECT count(*) FROM revision").fetch_one(store.pool()).await.unwrap(),
            "post" => sqlx::query_scalar("SELECT count(*) FROM post").fetch_one(store.pool()).await.unwrap(),
            "node" => sqlx::query_scalar("SELECT count(*) FROM node").fetch_one(store.pool()).await.unwrap(),
            _ => continue,
        };
        counts.push((table, n));
    }
    counts
}

#[tokio::test]
async fn the_losses_name_every_other_revision_and_every_post_of_the_point_and_change_nothing() {
    let (_dir, store, work) = orchard().await;
    // Abschnitt 1 „Der Schuppen“: Fassung 1 (Import, wird ersetzt), 2 Vorschlag verworfen, 3 Mensch (aktuell),
    // 4 offener Vorschlag; dazu zwei Beiträge. Abschnitt 0 bekommt einen fremden Beitrag und einen Vorschlag.
    let rejected = store
        .add_proposal(work, 1, "= Der Schuppen\n\nLeitern, Körbe und ein Fass.\n", "@lektor", "probe", "mit Fass")
        .await
        .unwrap();
    store.reject(work, 1, rejected).await.unwrap();
    store.save_human(work, 1, "= Der Schuppen\n\nLeitern, Körbe und die alte Sense.\n", HUMAN).await.unwrap().unwrap();
    let open = store
        .add_proposal(work, 1, "= Der Schuppen\n\nLeitern, Körbe, Sense und Wetzstein.\n", "@nathan", "probe", "Stein")
        .await
        .unwrap();
    let first = store.add_post(work, 1, &post("@nathan", "Wozu die Sense?")).await.unwrap();
    let second = store.add_post(work, 1, &post(HUMAN, "Für das hohe Gras unter den Bäumen.")).await.unwrap();
    store.add_post(work, 0, &post("@lektor", "Beitrag zu einem anderen Abschnitt.")).await.unwrap();
    store
        .add_proposal(work, 0, "= Die Obstwiese\n\nVier Reihen Apfelbäume.\n", "@lektor", "probe", "vier")
        .await
        .unwrap();

    let before = row_counts(&store).await;
    let losses = store.outline_delete_losses(work, 1).await.unwrap();
    assert_eq!(row_counts(&store).await, before, "die Funktion liest nur: kein Datensatz kommt hinzu oder fällt weg");
    assert_eq!(
        losses.revisions,
        vec![
            LostRevision {
                number: 1,
                author: HUMAN.to_owned(),
                proposal: false,
                // Wortlaut wie eingelesen: Der Import lässt die Leerzeile vor der nächsten Überschrift beim Abschnitt.
                body: "= Der Schuppen\n\nLeitern und Körbe.\n\n".to_owned(),
            },
            LostRevision {
                number: rejected,
                author: "@lektor".to_owned(),
                proposal: false,
                body: "= Der Schuppen\n\nLeitern, Körbe und ein Fass.\n".to_owned(),
            },
            LostRevision {
                number: open,
                author: "@nathan".to_owned(),
                proposal: true,
                body: "= Der Schuppen\n\nLeitern, Körbe, Sense und Wetzstein.\n".to_owned(),
            },
        ],
        "alle Fassungen außer der aktuellen, nach Nummer; nur der offene Vorschlag trägt proposal = true \
         (ein verworfener ist keiner mehr); die aktuelle Fassung 3 steht nicht dabei (sie nennt outline_delete selbst)"
    );
    assert_eq!(
        losses.posts,
        vec![
            LostPost { id: first, author: "@nathan".to_owned(), body: "Wozu die Sense?".to_owned() },
            LostPost { id: second, author: HUMAN.to_owned(), body: "Für das hohe Gras unter den Bäumen.".to_owned() },
        ],
        "alle Beiträge des Punkts in ihrer Reihenfolge – und keiner eines anderen Abschnitts"
    );

    // Ein Punkt ohne weitere Fassungen und Beiträge verliert nichts außer der aktuellen Fassung.
    assert_eq!(
        store.outline_delete_losses(work, 2).await.unwrap(),
        OutlineLosses::default(),
        "nur die aktuelle Fassung: keine Verluste zu melden"
    );

    // Was gemeldet wurde, ist genau das, was outline_delete danach entfernt.
    let lost_revisions = losses.revisions.len() as i64 + 1;
    let lost_posts = losses.posts.len() as i64;
    store.outline_delete(work, 1, HUMAN).await.unwrap();
    let after = row_counts(&store).await;
    let count = |rows: &[(String, i64)], table: &str| rows.iter().find(|(t, _)| t == table).unwrap().1;
    assert_eq!(count(&before, "post") - count(&after, "post"), lost_posts, "genau die gemeldeten Beiträge sind weg");
    assert_eq!(
        count(&before, "revision") - count(&after, "revision"),
        lost_revisions,
        "genau die gemeldeten Fassungen und die aktuelle sind weg"
    );
}

#[tokio::test]
async fn unknown_points_and_works_are_reported_like_everywhere_else() {
    let (_dir, store, work) = orchard().await;
    let err = store.outline_delete_losses(work, 99).await.unwrap_err();
    assert!(
        matches!(err, StoreError::UnknownNode { work: w, node: 99 } if w == work),
        "ein Punkt, den es nicht gibt: UnknownNode, nicht eine leere Liste (sonst sähe es nach „nichts verloren“ aus): {err:?}"
    );
    let err = store.outline_delete_losses(work + 1000, 0).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownWork(_)), "unbekanntes Werk: {err:?}");
}
