// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23a: findings – collective findings do not count in „k offen“ and are called „veraltet“ as soon as all origins
//! are decided; the list „Offen, älter als n Tage“ names its cap; cells of the matrix without a link carry their words
//! as hidden text. Work and findings freely invented.

use axum::http::StatusCode;
use baruch_store::{FindingKind, NewFinding, StatusChange};
use serde_json::json;

use crate::common;

async fn finding(
    server: &common::Server,
    work: i64,
    key: i64,
    body: &str,
    summary_of: &[String],
) -> baruch_store::Finding {
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: body.into(),
        sources: vec![],
        refs: vec![],
    };
    let post = server.state.store.add_post(work, 0, &post).await.unwrap();
    let new = NewFinding {
        work,
        key: Some(key),
        post: Some(post),
        thread_post: None,
        position: 0,
        persona: "@lektor",
        kind: FindingKind::Hint,
        body,
        quote: "",
        quote_check: "",
        similar_to: None,
        summary_of,
    };
    server.state.store.finding_insert(&new).await.unwrap()
}

async fn mill(server: &common::Server) -> (i64, i64) {
    let work = server.import("= Die Mühle\n\nDas Rad dreht sich langsam.\n", "muehle-23.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    (work, key)
}

#[tokio::test]
async fn a_summary_does_not_count_as_open_and_is_called_stale_once_its_origins_are_decided() {
    let server = common::open().await;
    let (work, key) = mill(&server).await;
    let a = finding(&server, work, key, "Hinweis: Das Rad braucht ein Geräusch.", &[]).await;
    let b = finding(&server, work, key, "Hinweis: „langsam“ doppelt sich.", &[]).await;
    let summary = finding(
        &server,
        work,
        key,
        "Sammlung: Geräusch am Rad, „langsam“ streichen.",
        &[a.uuid.clone(), b.uuid.clone()],
    )
    .await;

    let badges = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(
        badges["result"]["open_findings"], 2,
        "„k offen“ zählt die zwei Ursprünge, nicht zusätzlich den Sammelbefund: {badges}"
    );
    let (_, tree) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(tree.contains("2 offen"), "auch die Arbeitsseite zeigt 2 offen");
    assert!(!tree.contains("3 offen"), "nirgends steht die doppelte Zählung „3 offen“");

    let url = format!("/werke/{work}/befunde?befund={}", summary.id);
    let (_, before) = server.get(&url).await;
    assert!(!before.contains("veraltet (alle Ursprünge entschieden)"), "mit offenen Ursprüngen ist nichts veraltet");

    for (id, change) in [
        (a.id, StatusChange::Rejected { reason: "passt nicht zum Ton".into() }),
        (b.id, StatusChange::Done { reason: String::new() }),
    ] {
        server.state.store.finding_set_status(work, id, &change, "Ida").await.unwrap();
    }
    let (status, after) = server.get(&url).await;
    assert_eq!(status, StatusCode::OK, "Seite des Sammelbefunds lädt");
    assert!(
        after.contains("offen · veraltet (alle Ursprünge entschieden)"),
        "der offene Sammelbefund trägt den Zusatz „veraltet“, sobald alle Ursprünge entschieden sind: {after}"
    );
    let got = server.rpc("finding.get", json!({ "work": work, "id": summary.id })).await;
    assert_eq!(got["result"]["finding"]["stale_summary"], true, "auch RPC finding.get nennt es: {got}");
    assert_eq!(got["result"]["finding"]["status"], "open", "der Status selbst bleibt beim Menschen");
    let badges = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(badges["result"]["open_findings"], 0, "kein offener Befund zählt mehr: {badges}");
}

#[tokio::test]
async fn the_list_of_old_open_findings_says_when_it_is_capped() {
    let server = common::open().await;
    let (work, key) = mill(&server).await;
    let limit = baruch_server::findings::page::OLD_LIMIT as usize;
    for n in 0..limit + 3 {
        finding(&server, work, key, &format!("Hinweis {n}: Wort {n} prüfen."), &[]).await;
    }
    // Make all findings one week old (without sleeping).
    sqlx::query("UPDATE finding SET created = strftime('%Y-%m-%dT%H:%M:%fZ','now','-7 days') WHERE work_id = ?")
        .bind(work)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let (status, page) = server.get(&format!("/werke/{work}/befunde?older_than_days=3")).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    assert!(
        page.contains("data-old-truncated")
            && page.contains(&format!(
                "Es gibt {} offene Befunde, die älter sind; gezeigt sind die {limit} neuesten",
                limit + 3
            )),
        "über der Grenze nennt die Liste ihre Kappung mit beiden Zahlen (vorher still)"
    );

    // Below the limit there is no hint.
    let other = common::open().await;
    let (work, key) = mill(&other).await;
    finding(&other, work, key, "Hinweis: Nur einer.", &[]).await;
    let (_, page) = other.get(&format!("/werke/{work}/befunde?older_than_days=0")).await;
    assert!(!page.contains("data-old-truncated"), "ohne Kappung kein Hinweis");
}

#[tokio::test]
async fn matrix_cells_without_a_link_carry_their_words_as_hidden_text() {
    let server = common::open().await;
    let (work, key) = mill(&server).await;
    finding(&server, work, key, "Hinweis: Das Rad knarrt nicht.", &[]).await;
    let (_, page) = server.get(&format!("/werke/{work}/befunde")).await;
    let foot = &page[page.find("<tfoot>").expect("Summenzeile der Matrix")..];
    let foot = &foot[..foot.find("</tfoot>").unwrap()];
    assert!(
        foot.contains("<span class=\"visually-hidden\">") && foot.contains("<span aria-hidden=\"true\">"),
        "„Gesamt“ hat keinen Link: die Worte stehen als verborgener Text, die Zeichen sind für Vorleser stumm: {foot}"
    );
    let hidden = &foot[foot.find("<span class=\"visually-hidden\">").unwrap() + 30..];
    let hidden = &hidden[..hidden.find("</span>").unwrap()];
    assert!(hidden.contains("offen"), "der verborgene Text nennt die Zahlen in Worten: {hidden}");
    assert!(
        page.contains(".visually-hidden") || server.get("/static/app.css").await.1.contains(".visually-hidden"),
        "die Klasse zum Verbergen gibt es im ausgelieferten Stil"
    );
}
