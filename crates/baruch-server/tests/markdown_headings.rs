// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown-Überschriften beim Tippen werden Gliederung (Nutzerentscheidung 2026-11-07): `# Titel` bis
//! `###### Titel` im Text des Menschen wird beim Speichern zur Typst-Überschrift (`=` × Ebene, 4–6 → 3),
//! und die Gliederung gleicht sich ab wie bei einer getippten `=`-Zeile – über RPC und ohne JavaScript
//! über das Formular. Vorschläge von Assistenten bleiben, wie sie sind; übernimmt der Mensch einen, gilt
//! dieselbe Regel. Die Regel selbst (Tabelle mit allen Ausnahmen) prüft `baruch_typst::markdown`. Alle
//! Texte frei erfunden.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const HARBOUR: &str = "= Die Ankunft\n\nDie Fähre legt an.\n\n= Der Sturm\nWind aus Nordwest.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// (Titel, Ebene als Tiefe im Baum) in Baumreihenfolge und der aktuelle Text je Punkt.
async fn outline(server: &common::Server, work: i64) -> Vec<(String, usize, String)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded
        .nodes
        .iter()
        .map(|n| {
            let mut depth = 1;
            let mut parent = n.parent;
            while let Some(p) = parent {
                depth += 1;
                parent = loaded.nodes[p].parent;
            }
            let body = n.current_index().map(|i| n.revisions[i].body.clone()).unwrap_or_default();
            (n.title.clone(), depth, body)
        })
        .collect()
}

async fn harbour(server: &common::Server) -> i64 {
    server.import(HARBOUR, "hafen.typ").await.work
}

#[tokio::test]
async fn markdown_headings_typed_by_the_human_become_outline_points_over_rpc() {
    let server = common::open().await;
    let work = harbour(&server).await;
    let body =
        "# Die Ankunft am Steg\n\nDie Fähre legt an.\n\n## Das Logbuch\nErste Eintragung.\n\n#### Am Rand\nNotiz.\n";
    let saved = server.rpc("outline.save", json!({ "work": work, "node": 0, "body": body, "split": true })).await;
    result(&saved);
    let tree = outline(&server, work).await;
    assert_eq!(
        tree.iter().map(|(t, d, _)| (t.as_str(), *d)).collect::<Vec<_>>(),
        [("Die Ankunft am Steg", 1), ("Das Logbuch", 2), ("Am Rand", 3), ("Der Sturm", 1)],
        "`#` benennt das Kapitel um, `##` wird Abschnitt, `####` auf Ebene 3 begrenzt (Unterabschnitt)"
    );
    assert_eq!(tree[0].2, "= Die Ankunft am Steg\n\nDie Fähre legt an.\n\n", "Text mit Typst-Überschrift gespeichert");
    assert_eq!(tree[1].2, "== Das Logbuch\nErste Eintragung.\n\n", "`##` → `==`");
    assert_eq!(tree[2].2, "=== Am Rand\nNotiz.\n", "`####` → `===`");

    // outline.check sieht dieselben Überschriften (das Widget fragt vor dem Speichern).
    let check = server
        .rpc(
            "outline.check",
            json!({ "work": work, "node": 3, "body": "# Der Sturm\n\nWind.\n\n## Die Flaute\nStille.\n" }),
        )
        .await;
    assert_eq!(
        result(&check)["split"].as_array().map(Vec::len),
        Some(1),
        "die Prüfung zählt `## Die Flaute` als neuen Punkt: {check}"
    );

    // revision.save (roher Weg des Menschen): `# Titel` derselben Ebene benennt um.
    let saved =
        server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "# Der große Sturm\n\nWind.\n" })).await;
    result(&saved);
    let tree = outline(&server, work).await;
    assert_eq!(tree[3].0, "Der große Sturm", "Titel folgt der umgewandelten Überschrift");
    assert_eq!(tree[3].2, "= Der große Sturm\n\nWind.\n", "gespeichert als `=`-Zeile");
}

#[tokio::test]
async fn exceptions_stay_text_and_the_outline_is_unchanged() {
    let server = common::open().await;
    let work = harbour(&server).await;
    let body = "= Die Ankunft\n\n\\# keine Überschrift\n'# in Anführungszeichen'\n\"# auch nicht\"\n\
                #set text(lang: \"de\")\n#heading(level: 2)[Code bleibt Code]\n```\n# im Raw-Block\n```\n\
                // # im Kommentar\n/* \n# im Blockkommentar\n*/\n#quote[\n# im Inhaltsblock\n]\n";
    let saved = server.rpc("outline.save", json!({ "work": work, "node": 0, "body": body })).await;
    result(&saved);
    let tree = outline(&server, work).await;
    assert_eq!(tree.len(), 2, "keine neuen Punkte: {tree:?}");
    assert_eq!(tree[0].2, body, "der Text bleibt Zeichen für Zeichen, wie er getippt wurde");
}

#[tokio::test]
async fn without_javascript_the_form_converts_asks_before_splitting_and_then_saves() {
    let server = common::open().await;
    let work = harbour(&server).await;
    let body = "# Die Ankunft\n\nDie Fähre legt an.\n\n## Das Logbuch\nErste Eintragung.\n";
    let encoded: String = body
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let form = format!("method=outline.save&work={work}&node=0&body={encoded}");
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.clone()))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "ohne Bestätigung erst die Rückfrage (teilen): {html}");
    assert!(html.contains("Das Logbuch"), "die Rückfrage nennt den neuen Punkt aus `## Das Logbuch`: {html}");
    assert_eq!(outline(&server, work).await.len(), 2, "vor der Bestätigung nichts gespeichert");

    let (status, _) = server.post_form("/gliederung/act", &format!("{form}&split=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Bestätigung gespeichert, 303 zurück");
    let tree = outline(&server, work).await;
    assert_eq!(
        tree.iter().map(|(t, d, _)| (t.as_str(), *d)).collect::<Vec<_>>(),
        [("Die Ankunft", 1), ("Das Logbuch", 2), ("Der Sturm", 1)],
        "ohne JavaScript dieselbe Gliederung wie über RPC"
    );
    assert!(tree[1].2.starts_with("== Das Logbuch\n"), "gespeichert als Typst-Überschrift: {tree:?}");
}

#[tokio::test]
async fn proposals_stay_as_written_and_accepting_one_applies_the_rule() {
    let server = common::open().await;
    let work = harbour(&server).await;
    let proposal = "= Die Ankunft\n\nDie Fähre legt an, die Möwen schreien.\n\n## Am Kai\nTaue werden festgemacht.\n";
    let rev = server.state.store.add_proposal(work, 0, proposal, "@lektor", "probe", "Kai ergänzt").await.unwrap();
    let loaded = server.state.store.load_work(work).await.unwrap();
    assert_eq!(
        loaded.nodes[0].revision(rev).map(|r| r.body.as_str()),
        Some(proposal),
        "ein Vorschlag des Assistenten bleibt, wie er geschrieben ist (keine Umwandlung, keine neue Gliederung)"
    );
    assert_eq!(outline(&server, work).await.len(), 2, "Vorschlag ändert die Gliederung nicht");

    let accepted = server.rpc("revision.accept", json!({ "work": work, "node": 0, "rev": rev })).await;
    let answer = result(&accepted);
    assert_eq!(answer["markdown"], "Markdown-Überschriften umgewandelt.", "Antwort sagt es: {answer}");
    let tree = outline(&server, work).await;
    assert_eq!(
        tree.iter().map(|(t, d, _)| (t.as_str(), *d)).collect::<Vec<_>>(),
        [("Die Ankunft", 1), ("Am Kai", 2), ("Der Sturm", 1)],
        "beim Übernehmen durch den Menschen gilt die Regel: `## Am Kai` wird ein Abschnitt"
    );
    assert_eq!(tree[1].2, "== Am Kai\nTaue werden festgemacht.\n", "umgewandelt gespeichert");
    let loaded = server.state.store.load_work(work).await.unwrap();
    assert_eq!(
        loaded.nodes[0].revision(rev).map(|r| r.body.as_str()),
        Some(proposal),
        "der übernommene Vorschlag selbst bleibt im Revisionsbaum unverändert"
    );
}
