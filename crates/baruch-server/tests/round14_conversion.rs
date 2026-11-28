// SPDX-License-Identifier: AGPL-3.0-or-later
//! Follow-up to Round 14 (user question 8, decided 2026-11-07: „automatisch umwandeln“): parts with the old text
//! „= Teil …“ or an empty text get the part marking once at start.
//!
//! The database "before Round 14" is reproduced: Round 14 had no migration, so the same schema, only the data are old –
//! parts with a `= …` line (old seed, also a part renamed before Round 14), an empty part (this is how `outline.add`
//! created parts before Round 14) and parts with a text of their own. For this the store is opened without a server (no
//! start step), only `AppState::open` converts. All works and texts freely invented.

use crate::common;

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_core::{Author, NodeKind, RevisionStatus};
use baruch_store::{CONVERSION_NOTE, CONVERTED_KEY, Store};
use serde_json::{Value, json};

/// Work with work template; the parts are still there as `=` lines (made into parts below).
const FERRY: &str = "#set document(title: \"Das Jahr der Fähre\", author: \"Ilse Erdacht\")\n\
#set page(width: 10cm, height: 14cm)\n#set heading(numbering: \"1.1\")\n\n\
= Teil I – Am Ufer\n\n\
= Die Ankunft\nDie Fähre legt an, der Steg knarrt.\n\n\
== Das Logbuch\nErste Eintragung: Nebel über dem Sund.\n\n\
= Teil II – Das Meer\n\n\
= Der Sturm\nDie Wellen schlagen über die Reling.\n";

/// Work without work template with parts whose text the human wrote himself.
const ORCHARD: &str = "= Zwischenspiel\n\nEin eigener Satz, den niemand anfassen darf.\n\n\
= Der Garten\nÄpfel im Gras.\n\n\
= Andere Zeile\n\n\
= Die Ernte\nKörbe voller Birnen.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

async fn exec(store: &Store, sql: &str, binds: &[&str]) {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for b in binds {
        query = query.bind(*b);
    }
    query.execute(store.pool()).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Make the node with position `seq` a part or re-hang it (work `work`).
async fn set_node(store: &Store, work: i64, seq: i64, kind: &str, parent: Option<i64>, position: i64, number: &str) {
    sqlx::query("UPDATE node SET kind = ?, parent_seq = ?, position = ?, number = ? WHERE work_id = ? AND seq = ?")
        .bind(kind)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(work)
        .bind(seq)
        .execute(store.pool())
        .await
        .unwrap();
}

/// Create the database in the state before Round 14; returns (Fähre, Obstgarten).
async fn old_database(dir: &Path) -> (i64, i64) {
    let store = Store::open(dir).await.unwrap();
    let ferry = store.import_typst(FERRY, "faehre.typ", "Autor").await.unwrap().work;
    // seq 0 Teil I, 1 Ankunft, 2 Logbuch (below 1), 3 Teil II, 4 Sturm.
    set_node(&store, ferry, 0, "part", None, 0, "I").await;
    set_node(&store, ferry, 1, "chapter", Some(0), 0, "1").await;
    set_node(&store, ferry, 3, "part", None, 1, "II").await;
    set_node(&store, ferry, 4, "chapter", Some(3), 0, "2").await;
    // Teil II was renamed before Round 14: only the title changed, the text stayed „= Teil II – Das Meer“.
    exec(&store, "UPDATE node SET title = 'Teil II – Die Weite' WHERE seq = 3 AND work_id = ?", &[&ferry.to_string()])
        .await;
    // Empty part at the end, as `outline.add` created parts before Round 14 (revision 1 with empty text).
    let empty: i64 = sqlx::query_scalar(
        "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
         VALUES (?, 5, NULL, 2, 'III', 'Teil III – Nachklang', 'part') RETURNING id",
    )
    .bind(ferry)
    .fetch_one(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
         VALUES (?, 1, '', 'human', 'Autor', 'current', 'Gliederung: Teil angelegt')",
    )
    .bind(empty)
    .execute(store.pool())
    .await
    .unwrap();

    let orchard = store.import_typst(ORCHARD, "obstgarten.typ", "Autor").await.unwrap().work;
    // seq 0 Zwischenspiel (own text), 1 Garten, 2 „Andere Zeile“ as part „Am Ende“ (foreign line), 3 Ernte.
    set_node(&store, orchard, 0, "part", None, 0, "I").await;
    set_node(&store, orchard, 1, "chapter", Some(0), 0, "1").await;
    set_node(&store, orchard, 2, "part", None, 1, "II").await;
    set_node(&store, orchard, 3, "chapter", Some(2), 0, "2").await;
    exec(&store, "UPDATE node SET title = 'Am Ende' WHERE seq = 2 AND work_id = ?", &[&orchard.to_string()]).await;
    let marked: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
        .bind(CONVERTED_KEY)
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(marked.as_deref(), Some("1"), "eine frisch gesäte Datenbank setzt den Merker gleich mit");
    // A database from before Round 14 does not have the flag (it did not exist).
    sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(CONVERTED_KEY).execute(store.pool()).await.unwrap();
    store.pool().close().await;
    (ferry, orchard)
}

/// All revisions of a part: (number, text, status, author, note, parent).
type Rev = (u32, String, RevisionStatus, Author, String, Option<u32>);

async fn revisions(server: &common::Server, work: i64, title: &str) -> (i64, NodeKind, Vec<Rev>) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("kein Punkt „{title}“"));
    // The parent (`based_on`) is not in the loaded work: from SQLite.
    let parents: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT number, based_on FROM revision WHERE node_id = ? ORDER BY number")
            .bind(node.key)
            .fetch_all(server.state.store.pool())
            .await
            .unwrap();
    let revs = node
        .revisions
        .iter()
        .map(|r| {
            let parent = parents.iter().find(|(n, _)| *n == i64::from(r.number)).and_then(|(_, p)| *p);
            (r.number, r.body.clone(), r.status, r.author.clone(), r.note.clone(), parent.map(|p| p as u32))
        })
        .collect();
    (node.key, node.kind, revs)
}

async fn open_server(dir: tempfile::TempDir) -> common::Server {
    let state = Arc::new(baruch_server::AppState::open(dir.path()).await.expect("Datenordner öffnet"));
    common::Server { app: common::router(state.clone()), state, data: dir }
}

fn log(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "--format=%an|%s%n%b%n--"])
}

#[tokio::test]
async fn old_part_texts_become_part_markers_once_without_loss_and_survive_export_reimport_and_the_pdf() {
    let _serial = common::PDF_SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let (ferry, orchard) = old_database(dir.path()).await;
    let server = open_server(dir).await;
    let store = &server.state.store;
    let marker = |title: &str| format!("{}\n", baruch_typst::part_line(title));

    // Converted: old seed, renamed before Round 14 (the title of the part counts), empty.
    for (title, old_body) in [
        ("Teil I – Am Ufer", "= Teil I – Am Ufer\n\n"),
        ("Teil II – Die Weite", "= Teil II – Das Meer\n\n"),
        ("Teil III – Nachklang", ""),
    ] {
        let (_, kind, revs) = revisions(&server, ferry, title).await;
        assert_eq!(kind, NodeKind::Part, "{title}: bleibt ein Teil");
        assert_eq!(revs.len(), 2, "{title}: alte Fassung bleibt, eine neue kommt hinzu: {revs:?}");
        let (old, new) = (&revs[0], &revs[1]);
        assert_eq!(
            (old.1.as_str(), old.2),
            (old_body, RevisionStatus::Superseded),
            "{title}: alte Fassung verlustfrei erhalten, nur abgelöst"
        );
        assert_eq!(new.1, marker(title), "{title}: neue Fassung = Teil-Markierung mit dem Titel des Teils + Leerzeile");
        assert_eq!(new.2, RevisionStatus::Current, "{title}: die neue Fassung ist die aktuelle");
        assert_eq!(new.3, Author::Human { name: "Autor".into() }, "{title}: Entscheidung des Menschen");
        assert_eq!(new.4, CONVERSION_NOTE, "{title}: Herkunft „Umwandlung Runde 14 (Nutzerentscheidung)“");
        assert_eq!(new.5, Some(old.0), "{title}: Elternteil im Revisionsbaum ist die alte Fassung");
    }

    // Not touched: own text, foreign `=` line; parts without a revision (sample work) likewise.
    let mut kept = Vec::new();
    for title in ["Zwischenspiel", "Am Ende"] {
        let (key, kind, revs) = revisions(&server, orchard, title).await;
        assert_eq!(kind, NodeKind::Part, "{title}: Teil");
        assert_eq!(revs.len(), 1, "{title}: eigener Text bleibt unverändert, keine neue Fassung: {revs:?}");
        kept.push(key);
    }
    // The sample work of an old database: empty parts (seed since Round 5) are converted as well.
    let sample = store.default_work().await.unwrap();
    let loaded = store.load_work(sample).await.unwrap();
    let sample_parts: Vec<_> = loaded.nodes.iter().filter(|n| n.kind == NodeKind::Part).collect();
    assert_eq!(sample_parts.len(), 2, "Beispielwerk hat zwei Teile");
    for part in sample_parts {
        let current = &part.revisions[part.current_index().unwrap()];
        assert_eq!(current.body, marker(&part.title), "Beispielwerk: {} trägt jetzt die Markierung", part.title);
        assert_eq!(part.revisions[0].body, "", "Beispielwerk: leere Fassung 1 bleibt erhalten");
    }

    // Mirror: exactly one log commit for the Fähre, none for the Obstgarten.
    let ferry_slug = store.work_slug(ferry).await.unwrap();
    let orchard_slug = store.work_slug(orchard).await.unwrap();
    let mirrored_before = log(&server, &ferry_slug);
    if let Some(history) = &mirrored_before {
        assert_eq!(
            history
                .matches("Umwandlung Runde 14 (Nutzerentscheidung): 3 Teile auf die Teil-Markierung umgestellt")
                .count(),
            1,
            "ein Commit mit Betreff und Zahl: {history}"
        );
        assert!(history.contains("Baruch-Action: parts-converted"), "Trailer der Aktion: {history}");
        assert!(history.contains("Baruch-Origin: Umwandlung Runde 14 (Nutzerentscheidung)"), "Herkunft: {history}");
        assert!(history.contains("Teil II – Die Weite (Knoten 3): Fassung 1 (alte =-Zeile) → Fassung 2"), "{history}");
        assert!(history.contains("Teil III – Nachklang (Knoten 5): Fassung 1 (leer) → Fassung 2"), "{history}");
        let orchard_log = log(&server, &orchard_slug).unwrap();
        assert!(!orchard_log.contains("Umwandlung"), "Werk ohne Umwandlung bekommt keinen Commit: {orchard_log}");
        let files: Vec<String> = std::fs::read_dir(server.mirror_dir_of(&ferry_slug).join("abschnitte"))
            .unwrap()
            .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
            .collect();
        assert_eq!(
            files.iter().filter(|t| t.contains("<baruch-part>")).count(),
            3,
            "der Spiegel trägt die drei Markierungen"
        );
    }

    // Idempotent: a second call does nothing (flag); without the flag it would find nothing to convert any more either.
    let again = store.convert_legacy_parts("Autor").await.unwrap();
    assert!(!again.ran && again.converted.is_empty(), "Merker gesetzt: kein zweiter Lauf: {again:?}");
    sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(CONVERTED_KEY).execute(store.pool()).await.unwrap();
    let rerun = store.convert_legacy_parts("Autor").await.unwrap();
    assert!(rerun.ran, "ohne Merker läuft der Schritt");
    assert!(rerun.converted.is_empty(), "eine Markierung ist weder leer noch eine =-Zeile: {rerun:?}");
    kept.sort_unstable();
    let mut rerun_kept = rerun.kept.clone();
    rerun_kept.sort_unstable();
    assert_eq!(rerun_kept, kept, "gemeldet werden nur die Knoten-ids der Teile mit eigenem Text");

    // Export → import → export byte-identical, the parts come back as parts.
    let exported = store.export_typst(ferry).await.unwrap();
    for title in ["Teil I – Am Ufer", "Teil II – Die Weite", "Teil III – Nachklang"] {
        assert!(
            exported.contains(&baruch_typst::part_line(title)),
            "Export trägt die Markierung „{title}“: {exported}"
        );
    }
    assert!(!exported.contains("= Teil"), "keine alte Teilzeile mehr im Export: {exported}");
    let source = exported.replace("Das Jahr der Fähre", "Das Jahr der Fähre wieder");
    let again = server.import(&source, "faehre-wieder.typ").await.work;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(again).await.unwrap()),
        baruch_typst::xref::strip_key_labels(&source),
        "Export → Import → Export byte-gleich (bis auf die Sprungmarken)"
    );
    let parts: Vec<String> = store
        .load_work(again)
        .await
        .unwrap()
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Part)
        .map(|n| n.title.clone())
        .collect();
    assert_eq!(parts, ["Teil I – Am Ufer", "Teil II – Die Weite", "Teil III – Nachklang"], "Teile beim Re-Import");

    // Typesetting and book PDF with part pages (right-hand pages).
    let typeset = baruch_server::typeset::typeset(&server.state, ferry).await.unwrap();
    let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
    assert!(errors.is_empty(), "Satz ohne Fehler: {errors:?}");
    for title in ["Teil I – Am Ufer", "Teil II – Die Weite"] {
        let (key, _, _) = revisions(&server, ferry, title).await;
        let page = typeset.page_of(Some(key));
        assert_eq!(page % 2, 1, "{title}: Teil-Seite ist eine rechte Seite: {page}");
    }
    let pdf = server.rpc("work.pdf", json!({ "work": ferry })).await;
    assert_eq!(
        result(&pdf)["pages"].as_u64().unwrap() as usize,
        typeset.page_count(),
        "Buch-PDF hat die Seiten des Satzes samt Teil-Seiten"
    );
    let response = tower::ServiceExt::oneshot(
        server.app.clone(),
        Request::get(format!("/werke/{ferry}/pdf")).body(Body::empty()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Buch-PDF über die Seite");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/pdf", "als PDF");

    // Restart: no new revision, no new commit.
    let revs_before = revisions(&server, ferry, "Teil I – Am Ufer").await.2.len();
    let common::Server { data, state, app } = server;
    drop(app);
    drop(state);
    let server = open_server(data).await;
    assert_eq!(
        revisions(&server, ferry, "Teil I – Am Ufer").await.2.len(),
        revs_before,
        "Neustart: keine neue Fassung"
    );
    if let Some(before) = mirrored_before {
        let after = log(&server, &ferry_slug).unwrap();
        assert_eq!(
            after.matches("Umwandlung Runde 14").count(),
            before.matches("Umwandlung Runde 14").count(),
            "Neustart: kein zweiter Protokoll-Commit"
        );
    }
}
