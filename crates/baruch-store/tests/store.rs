// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahme von baruch-store (Vertrag B1, Abschnitte 3, 4 und 6) auf Temp-Ordnern.

use baruch_core::{Author, Post, RevisionStatus, TextRef, sample};
use baruch_store::{Change, ChangeKind, Store, StoreError, slug};

async fn fresh() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

/// Werk als JSON ohne die festen Schlüssel (`key` = Datenbank-id; die Beispieldaten im Speicher
/// haben keine) und ohne id und Zeitpunkt der Beiträge (vergibt die Datenbank, Runde 8), damit sich
/// geladenes Werk und Beispielwerk Feld für Feld vergleichen lassen.
fn without_keys(work: &baruch_core::Work) -> serde_json::Value {
    let mut value = serde_json::to_value(work).unwrap();
    for node in value["nodes"].as_array_mut().unwrap() {
        node.as_object_mut().unwrap().remove("key");
        for post in node["posts"].as_array_mut().unwrap() {
            let post = post.as_object_mut().unwrap();
            post.remove("id");
            post.remove("created");
        }
    }
    value
}

fn statuses(work: &baruch_core::Work, node: usize) -> Vec<RevisionStatus> {
    work.nodes[node].revisions.iter().map(|r| r.status).collect()
}

fn currents(work: &baruch_core::Work, node: usize) -> usize {
    statuses(work, node).iter().filter(|s| **s == RevisionStatus::Current).count()
}

#[tokio::test]
async fn open_creates_db_and_round_trips_sample() {
    let (dir, store, work) = fresh().await;
    assert!(dir.path().join("baruch.sqlite").is_file(), "die Datenbankdatei liegt im Datenordner");

    let loaded = store.load_work(work).await.unwrap();
    let expected = sample::work();
    // Feldweise über serde: Knoten (id/seq, parent, number, title, kind, findings), Revisionen
    // (number, body, author, status, note), Beiträge samt sources und refs, Personas.
    assert_eq!(without_keys(&loaded), without_keys(&expected), "das Beispielwerk kommt unverändert zurück");
    let keys: Vec<i64> = loaded.nodes.iter().map(|n| n.key).collect();
    assert!(keys.iter().all(|&k| k > 0), "jeder geladene Knoten trägt seinen festen Schlüssel: {keys:?}");
    assert_eq!(loaded.nodes.len(), expected.nodes.len(), "alle Knoten geladen");
    assert!(
        loaded.nodes[5].posts[1].refs[0]
            == TextRef { node: 4, rev: 2, quote: "Langmut heißt nicht, Unrecht zu dulden.".into() },
        "Verweise im Beitrag bleiben"
    );
    assert_eq!(loaded.nodes[2].posts[3].sources, expected.nodes[2].posts[3].sources, "Quellen der Beiträge bleiben");
    assert_eq!(loaded.lector().map(|p| &p.handle), expected.lector().map(|p| &p.handle), "derselbe Lektor");
    assert_eq!(loaded.lector_choice, baruch_core::LectorChoice::Fixed("@lektor".into()), "Vorgabe: fest @lektor");
    assert_eq!(loaded.lector().unwrap().handle, "@lektor", "Lektor ist fest @lektor (hat auch den größten Kontext)");

    let works = store.works().await.unwrap();
    assert_eq!(works.len(), 1, "genau ein Werk gesät");
    assert_eq!(works[0].1, slug(&expected.title), "slug aus dem Titel");
    assert_eq!(store.work_slug(work).await.unwrap(), slug(&expected.title), "slug über die id");
}

#[tokio::test]
async fn second_open_does_not_seed_again() {
    let dir = tempfile::tempdir().unwrap();
    let first = Store::open(dir.path()).await.unwrap();
    // Eine Migration eines anderen Migrators (Bibliothek, 0101) in derselben Tabelle darf den
    // Kernmigrator nicht stören (ignore_missing).
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (101, 'library', 1, x'00', 0)",
    )
    .execute(first.pool())
    .await
    .unwrap();
    first.pool().close().await;
    let second = Store::open(dir.path()).await.unwrap();
    assert_eq!(second.works().await.unwrap().len(), 1, "kein zweites Werk gesät");
    let personas: i64 = sqlx::query_scalar("SELECT count(*) FROM persona").fetch_one(second.pool()).await.unwrap();
    assert_eq!(personas, sample::work().personas.len() as i64, "keine doppelten Personas");
    let migrations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations").fetch_one(second.pool()).await.unwrap();
    // Jede Kernmigration genau einmal, dazu die fremde 0101.
    let core = baruch_store::MIGRATOR.iter().count() as i64;
    assert_eq!(migrations, core + 1, "Kernmigrationen einmal, 0101 bleibt stehen");
}

#[tokio::test]
async fn accept_makes_proposal_the_only_current() {
    let (_dir, store, work) = fresh().await;
    let change = store.accept(work, 2, 3).await.unwrap();
    assert_eq!(
        change,
        Change {
            work,
            node: 2,
            rev: 3,
            author: Author::Assistant { persona: "@lektor".into(), model: "google/gemma-4-26b-a4b".into() },
            kind: ChangeKind::Accepted,
            before: None,
            reparented: vec![],
            amend_seq: None,
        },
        "Übernehmen meldet den Vorschlag als neue aktuelle Fassung mit seinem Autor"
    );
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 2),
        [RevisionStatus::Superseded, RevisionStatus::Superseded, RevisionStatus::Current],
        "die alte aktuelle Fassung ist ersetzt"
    );
    assert_eq!(currents(&after, 2), 1, "genau eine aktuelle Fassung");

    let again = store.accept(work, 2, 3).await;
    assert!(matches!(again, Err(StoreError::Refused(m)) if m == "Nur Vorschläge lassen sich übernehmen."), "{again:?}");
    assert!(
        matches!(store.accept(work, 2, 1).await, Err(StoreError::Refused(_))),
        "eine ersetzte Fassung ist kein Vorschlag"
    );
    let unchanged = store.load_work(work).await.unwrap();
    assert_eq!(
        serde_json::to_value(&unchanged).unwrap(),
        serde_json::to_value(&after).unwrap(),
        "abgelehnte Züge ändern nichts"
    );
}

#[tokio::test]
async fn concurrent_accepts_take_effect_once() {
    let (_dir, store, work) = fresh().await;
    let (a, b) = tokio::join!(store.accept(work, 2, 3), store.accept(work, 2, 3));
    let ok = [&a, &b].iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 1, "{a:?} / {b:?}");
    assert!(
        [&a, &b].iter().any(|r| matches!(r, Err(StoreError::Refused(_)))),
        "der zweite Aufruf findet keinen Vorschlag mehr"
    );
    assert_eq!(currents(&store.load_work(work).await.unwrap(), 2), 1, "genau eine aktuelle Fassung");
}

#[tokio::test]
async fn failing_step_inside_accept_rolls_back() {
    let (_dir, store, work) = fresh().await;
    let before = store.load_work(work).await.unwrap();
    // Der zweite Schritt (Vorschlag → current) scheitert; der erste (alte current → superseded)
    // ist dann schon ausgeführt und muss zurückgerollt werden.
    sqlx::query(
        "CREATE TRIGGER test_fail_accept BEFORE UPDATE OF status ON revision WHEN NEW.status = 'current' \
         BEGIN SELECT RAISE(ABORT, 'Testfehler'); END",
    )
    .execute(store.pool())
    .await
    .unwrap();
    let result = store.accept(work, 2, 3).await;
    assert!(matches!(&result, Err(StoreError::Db(e)) if e.to_string().contains("Testfehler")), "{result:?}");

    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "zurückgerollt: alles wie vorher"
    );
    assert_eq!(
        statuses(&after, 2),
        [RevisionStatus::Superseded, RevisionStatus::Current, RevisionStatus::Proposed],
        "Status wie vorher"
    );

    sqlx::query("DROP TRIGGER test_fail_accept").execute(store.pool()).await.unwrap();
    assert_eq!(store.accept(work, 2, 3).await.unwrap().rev, 3, "ohne Störung gelingt das Übernehmen");
}

#[tokio::test]
async fn reject_keeps_current() {
    let (_dir, store, work) = fresh().await;
    assert_eq!(store.reject(work, 10, 2).await.unwrap(), 1, "Verwerfen meldet die aktuelle Fassung");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 10),
        [RevisionStatus::Current, RevisionStatus::Rejected],
        "die aktuelle Fassung bleibt"
    );
    let again = store.reject(work, 10, 2).await;
    assert!(
        matches!(again, Err(StoreError::Refused(m)) if m == "Nur Vorschläge lassen sich verwerfen."),
        "schon verworfen"
    );
    assert!(
        matches!(store.accept(work, 10, 2).await, Err(StoreError::Refused(_))),
        "ein verworfener Vorschlag lässt sich nicht übernehmen"
    );
}

#[tokio::test]
async fn save_human_appends_current_and_skips_unchanged() {
    let (_dir, store, work) = fresh().await;
    let change = store.save_human(work, 0, "= Vorwort\r\n\r\nNeu.\r\n", "Autor").await.unwrap();
    assert_eq!(
        change,
        Some(Change {
            work,
            node: 0,
            rev: 2,
            author: Author::Human { name: "Autor".into() },
            kind: ChangeKind::Saved,
            before: None,
            reparented: vec![],
            amend_seq: None,
        }),
        "Speichern meldet die neue Fassung vom Menschen"
    );
    let after = store.load_work(work).await.unwrap();
    let node = &after.nodes[0];
    assert_eq!(
        statuses(&after, 0),
        [RevisionStatus::Superseded, RevisionStatus::Current],
        "die neue Fassung ist aktuell"
    );
    assert_eq!(node.revisions[1].body, "= Vorwort\n\nNeu.\n", "CRLF wird LF");
    assert_eq!(node.revisions[1].author, Author::Human { name: "Autor".into() }, "die Fassung stammt vom Menschen");
    let kind: String = sqlx::query_scalar(
        "SELECT r.author_kind FROM revision r JOIN node n ON n.id = r.node_id WHERE n.seq = 0 AND r.number = 2",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(kind, "human", "Autor in der Datenbank ist der Mensch");

    // Gleicher Text (auch mit CRLF) → nichts.
    assert_eq!(
        store.save_human(work, 0, "= Vorwort\r\n\r\nNeu.\r\n", "Autor").await.unwrap(),
        None,
        "gleicher Text mit CRLF legt nichts an"
    );
    assert_eq!(
        store.save_human(work, 0, "= Vorwort\n\nNeu.\n", "Autor").await.unwrap(),
        None,
        "gleicher Text legt nichts an"
    );
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].revisions.len(), 2, "weiter zwei Fassungen");

    // Speichern bei offenem Vorschlag: neue Nummer hinter dem Vorschlag, Vorschlag bleibt offen.
    let change = store.save_human(work, 2, "= Einleitung\n\nEigene Fassung\n", "Autor").await.unwrap().unwrap();
    assert_eq!(change.rev, 4, "neue Nummer hinter dem Vorschlag");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 2),
        [RevisionStatus::Superseded, RevisionStatus::Superseded, RevisionStatus::Proposed, RevisionStatus::Current],
        "der Vorschlag bleibt offen"
    );
}

#[tokio::test]
async fn add_proposal_never_becomes_current() {
    let (_dir, store, work) = fresh().await;
    let number = store
        .add_proposal(work, 4, "== Erste Anzeichen\r\n\r\nKürzer.\r\n", "@lektor", "qwen3:4b", "gekürzt")
        .await
        .unwrap();
    assert_eq!(number, 3, "nächste freie Nummer");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(
        statuses(&after, 4),
        [RevisionStatus::Superseded, RevisionStatus::Current, RevisionStatus::Proposed],
        "der Vorschlag wird nicht aktuell"
    );
    let proposal = &after.nodes[4].revisions[2];
    assert_eq!(
        proposal.author,
        Author::Assistant { persona: "@lektor".into(), model: "qwen3:4b".into() },
        "Autor ist die Persona mit Modell"
    );
    assert_eq!(proposal.note, "gekürzt", "Notiz bleibt");
    assert_eq!(proposal.body, "== Erste Anzeichen\n\nKürzer.\n", "CRLF wird LF");
    let based_on: Option<i64> = sqlx::query_scalar(
        "SELECT r.based_on FROM revision r JOIN node n ON n.id = r.node_id WHERE n.seq = 4 AND r.number = 3",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(based_on, Some(2), "Basis ist die aktuelle Fassung");
}

#[tokio::test]
async fn add_post_appends_with_sources_and_refs() {
    let (_dir, store, work) = fresh().await;
    let post = Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "  Danke, übernehme ich.  \n".into(),
        sources: vec!["Lutherbibel 1912, Röm 12,2".into()],
        refs: vec![TextRef { node: 4, rev: 2, quote: "Langmut".into() }],
    };
    let id = store.add_post(work, 2, &post).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    let last = after.nodes[2].posts.last().unwrap();
    assert_eq!(after.nodes[2].posts.len(), 5, "der Beitrag ist angehängt");
    assert!(id > 0, "add_post gibt die neue id zurück (Runde 8): {id}");
    assert_eq!(last.id, id, "load_work trägt dieselbe id im Beitrag");
    assert!(last.created.starts_with("20") && last.created.ends_with('Z'), "Zeitpunkt in ISO-Form: {}", last.created);
    assert!(
        after.nodes[2].posts.iter().filter(|p| p.id == id).count() == 1
            && after.nodes[2].posts.iter().all(|p| p.id > 0),
        "jeder gespeicherte Beitrag hat eine eigene id"
    );
    assert_eq!(last.body, "Danke, übernehme ich.", "Text getrimmt");
    assert_eq!(last.sources, post.sources, "Quellen bleiben");
    assert_eq!(last.refs, post.refs, "Verweise bleiben");

    let empty = Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: " \n ".into(),
        sources: vec![],
        refs: vec![],
    };
    let result = store.add_post(work, 2, &empty).await;
    assert!(matches!(&result, Err(StoreError::EmptyPost)), "leerer Beitrag abgelehnt");
    assert_eq!(result.unwrap_err().to_string(), "Leerer Beitrag.", "deutsche Meldung");
}

#[tokio::test]
async fn triggers_guard_origin_current_and_the_one_current_index() {
    let (_dir, store, _work) = fresh().await;
    let pool = store.pool();
    // Seit 0004 darf der Mensch löschen, aber nie die aktuelle Fassung.
    let delete = sqlx::query("DELETE FROM revision WHERE status = 'current'").execute(pool).await.unwrap_err();
    assert!(delete.to_string().contains("Die aktuelle Fassung lässt sich nicht löschen"), "{delete}");
    // Herkunft bleibt unveränderlich, auch wenn der Text sich ändern darf.
    let update = sqlx::query("UPDATE revision SET author_name = 'Fremd'").execute(pool).await.unwrap_err();
    assert!(update.to_string().contains("Herkunft einer Revision ist unveränderlich"), "{update}");
    // Eine zweite current im selben Abschnitt: superseded → current ist jetzt ein erlaubter Übergang,
    // also muss der Teilindex sie verhindern.
    let second = sqlx::query(
        "UPDATE revision SET status = 'current' WHERE status = 'superseded' \
         AND node_id = (SELECT id FROM node WHERE seq = 2)",
    )
    .execute(pool)
    .await
    .unwrap_err();
    assert!(second.to_string().contains("UNIQUE"), "Teilindex muss eine zweite current verhindern: {second}");
    // Auch ein neu eingefügter zweiter current-Eintrag scheitert am Teilindex.
    let insert = sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, status) \
         SELECT id, 99, 'x', 'human', 'Autor', 'current' FROM node WHERE seq = 2",
    )
    .execute(pool)
    .await
    .unwrap_err();
    assert!(insert.to_string().contains("UNIQUE"), "Teilindex muss eine zweite current verhindern: {insert}");
    // Nichts davon hat etwas geändert.
    assert_eq!(
        without_keys(&store.load_work(store.default_work().await.unwrap()).await.unwrap()),
        without_keys(&sample::work()),
        "abgelehnte Anweisungen hinterlassen keine Spur"
    );
}

#[tokio::test]
async fn mirror_snapshot_follows_tree_order() {
    let (_dir, store, work) = fresh().await;
    // Ein Abschnitt, der nach seq hinten steht, im Baum aber unter Teil I hinter Kapitel 2 hängt.
    let node_id: i64 = sqlx::query_scalar(
        "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
         VALUES (?, 11, 1, 2, '2a', 'Nachtrag Ä', 'chapter') RETURNING id",
    )
    .bind(work)
    .fetch_one(store.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO revision (node_id, number, body, author_kind, author_name, status) VALUES (?, 1, '= Nachtrag\n', 'human', 'Autor', 'current')")
        .bind(node_id)
        .execute(store.pool())
        .await
        .unwrap();
    store.accept(work, 2, 3).await.unwrap();

    let files = store.mirror_snapshot(work).await.unwrap();
    let loaded = store.load_work(work).await.unwrap();
    let seqs = [0, 1, 2, 3, 4, 5, 11, 6, 7, 8, 9, 10];
    let paths: Vec<String> =
        seqs.iter().map(|&s| format!("abschnitte/{s:03}-{}.typ", slug(&loaded.nodes[s].title))).collect();
    assert_eq!(paths[2], "abschnitte/002-einleitung.typ", "Dateiname aus seq und Titel");
    assert_eq!(paths[6], "abschnitte/011-nachtrag-ae.typ", "Umlaute im Dateinamen umschrieben");

    assert_eq!(files[0].path, "main.typ", "main.typ zuerst");
    let mut expected_main = format!("// Baruch: {}\n\n", loaded.title);
    for p in &paths {
        expected_main.push_str(&format!("#include \"{p}\"\n"));
    }
    assert_eq!(files[0].contents, expected_main, "main.typ bindet alle Abschnitte in Baumreihenfolge ein");
    assert_eq!(files.len(), seqs.len() + 1, "eine Datei je Abschnitt und main.typ");
    for (file, (&seq, path)) in files[1..].iter().zip(seqs.iter().zip(&paths)) {
        assert_eq!(&file.path, path, "Abschnittsdatei in Baumreihenfolge");
        let node = &loaded.nodes[seq];
        let current = &node.revisions[node.current_index().unwrap()].body;
        // Seit Runde 14 (Vertrag §4) projiziert der Spiegel „ohne Nummer“ (hier das Vorwort, seq 0) als
        // Markierung in der ersten Überschriftszeile; die DB bleibt unverändert.
        let expected =
            if seq == 0 { baruch_typst::set_numbered_line(current, false).unwrap() } else { current.clone() };
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&file.contents),
            expected,
            "Datei = aktuelle Fassung (projiziert, ohne Sprungmarke)"
        );
    }
    assert!(files[3].contents.contains("Notrufnummern"), "übernommener Vorschlag im Spiegel");
}

#[test]
fn slug_rules() {
    assert_eq!(slug("Das heimliche Muster – Ä"), "das-heimliche-muster-ae", "Umlaut und Gedankenstrich umgeschrieben");
    assert_eq!(slug("Straße, Öl & Übel!"), "strasse-oel-uebel", "ß, Umlaute und Satzzeichen umgeschrieben");
    assert_eq!(slug("  --Teil II–Lösen--  "), "teil-ii-loesen", "Ränder ohne Bindestrich");
    assert_eq!(slug(""), "werk", "leer → werk");
    assert_eq!(slug("– … –"), "werk", "nur Satzzeichen → werk");
    let long = slug("Mövenoog – ein ganzes Jahr auf der kleinen Insel");
    assert_eq!(long, "moevenoog-ein-ganzes-jahr-auf-der", "an der letzten Wortgrenze innerhalb von 40 Zeichen gekürzt");
    let exact = format!("{}-{}", "a".repeat(20), "b".repeat(19));
    assert_eq!(slug(&format!("{exact} c")), exact, "endet das Wort genau an Stelle 40, bleibt es ganz");
    let cut = slug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa b");
    assert_eq!(cut, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "kein Bindestrich am Ende nach dem Kürzen");
    let word = slug(&"x".repeat(60));
    assert_eq!(word, "x".repeat(40), "ein einzelnes langes Wort wird hart auf 40 Zeichen gekürzt");
    let tail = slug(&format!("{} {}", "y".repeat(45), "z"));
    assert_eq!(tail, "y".repeat(40), "langes erstes Wort ohne Grenze davor: hart gekürzt");
    for s in [long, cut, word, tail] {
        assert!(s.len() <= 40 && !s.starts_with('-') && !s.ends_with('-'), "höchstens 40, kein '-' am Rand: {s}");
    }
}

#[test]
fn default_data_dir_ends_in_baruch() {
    let dir = baruch_store::default_data_dir();
    assert!(dir.ends_with("baruch"), "Ordner heißt baruch: {}", dir.display());
    if let Some(local) = dirs::data_local_dir() {
        assert_eq!(dir, local.join("baruch"), "Vorgabe liegt unter data_local_dir (Windows: Local, nicht Roaming)");
    }
}

#[tokio::test]
async fn unknown_node_revision_and_work_have_german_messages() {
    let (_dir, store, work) = fresh().await;

    let err = store.accept(work, 99, 1).await.unwrap_err();
    assert!(matches!(err, StoreError::UnknownNode { node: 99, .. }), "Abschnitt 99 gibt es nicht: {err:?}");
    assert_eq!(err.to_string(), "unbekannter Abschnitt 99", "Meldung auf Deutsch mit der seq");

    let err = store.accept(work, 2, 9).await.unwrap_err();
    assert!(
        matches!(&err, StoreError::UnknownRevision { node: 2, rev: 9, existing } if existing == &[1, 2, 3]),
        "Abschnitt 2 hat die Fassungen 1–3: {err:?}"
    );
    assert_eq!(
        err.to_string(),
        "Fassung 9 gibt es nicht; vorhanden: 1, 2, 3",
        "die Meldung zählt die vorhandenen Nummern auf"
    );
    assert!(
        matches!(store.reject(work, 2, 0).await, Err(StoreError::UnknownRevision { rev: 0, .. })),
        "Nummer 0 gibt es nie"
    );
    assert!(
        matches!(store.save_human(work, 42, "x", "Autor").await, Err(StoreError::UnknownNode { node: 42, .. })),
        "Speichern in einen unbekannten Abschnitt"
    );
    assert!(
        matches!(store.add_proposal(work, 42, "x", "@lektor", "m", "").await, Err(StoreError::UnknownNode { .. })),
        "Vorschlag in einen unbekannten Abschnitt"
    );

    let err = store.load_work(work + 41).await.unwrap_err();
    assert_eq!(err.to_string(), format!("unbekanntes Werk {}", work + 41), "Meldung nennt die Werk-id");
    assert!(
        matches!(store.accept(work + 41, 2, 3).await, Err(StoreError::UnknownWork(_))),
        "unbekanntes Werk geht vor unbekanntem Abschnitt"
    );
    assert!(
        matches!(store.mirror_snapshot(work + 41).await, Err(StoreError::UnknownWork(_))),
        "Schnappschuss eines unbekannten Werks"
    );
    assert!(
        matches!(store.work_slug(work + 41).await, Err(StoreError::UnknownWork(_))),
        "slug eines unbekannten Werks"
    );
}
