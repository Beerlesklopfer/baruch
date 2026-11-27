// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsmanager im Store (Vertrag Runde 4, Abschnitte 1, 2 und 3c): Migration 0006 auf einer
//! Datenbank im Stand davor, Nummern nach Werkvorlage und Einstellung, Verschieben, Ebene ändern,
//! Umbenennen, Anlegen, Löschen, Überschriften im Text. Echte SQLite-Datei im Temp-Ordner, eigene
//! kleine Typst-Texte (unten im Test).

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use baruch_core::{Post, RevisionStatus, TextRef, Work, sample};
use baruch_store::{OutlineAction, Place, Restructure, Store, StoreError, WorkId};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Eigenes kleines Buch ohne Nummerierungsregel (Vorgabe 1.1).
const GARDEN: &str = "\
#set document(title: \"Gliederungsprobe\", author: \"Test\")
#set text(lang: \"de\")

= Frühling

Die Beete.

== Aussaat <aussaat>

Radieschen.

=== Tiefe

Zwei Zentimeter.

= Sommer

Gießen.

= Herbst

Ernte.
";

/// Eigenes Buch mit römischer Nummerierung und maskierten Überschriften.
const MASKED: &str = "\
#set document(title: \"Maskiertes Buch\", author: \"Test\")
#set heading(numbering: \"I.1\")

= Anna\\'s Laube

Text.

== \\#1 und \\*Stern\\*

Mehr.

= Pfad C:\\\\Daten

Schluss.
";

const HUMAN: &str = "Autor";

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn garden() -> (tempfile::TempDir, Store, WorkId) {
    let (dir, store) = fresh().await;
    let work = store.import_typst(GARDEN, "garten.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

fn titles(work: &Work) -> Vec<&str> {
    work.nodes.iter().map(|n| n.title.as_str()).collect()
}

fn numbers(work: &Work) -> Vec<&str> {
    work.nodes.iter().map(|n| n.number.as_str()).collect()
}

fn current_body(work: &Work, seq: usize) -> &str {
    let node = &work.nodes[seq];
    &node.revisions[node.current_index().expect("aktuelle Fassung")].body
}

/// Ebene je Knoten aus Art und Elternteil (wie der Store sie sieht).
fn levels(work: &Work) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for node in &work.nodes {
        let level = match node.kind {
            baruch_core::NodeKind::Part => 0,
            baruch_core::NodeKind::Chapter => 1,
            baruch_core::NodeKind::Section => match node.parent.map(|p| out[p]) {
                Some(p) if p >= 2 => 3,
                _ => 2,
            },
        };
        out.push(level);
    }
    out
}

/// Regeln nach jeder Änderung: seq lückenlos in Baumreihenfolge (prüft `load_work`), Elternteil =
/// nächster Vorgänger mit kleinerer Ebene, genau eine aktuelle Fassung je Abschnitt, Export = Text
/// in Baumreihenfolge und als Typst zerlegbar in dieselbe Gliederung.
async fn assert_consistent(store: &Store, work: WorkId) -> Work {
    let loaded = store.load_work(work).await.expect("Werk lädt (seq lückenlos, genau eine current)");
    let levels = levels(&loaded);
    for (index, node) in loaded.nodes.iter().enumerate() {
        let expected = (0..index).rev().find(|&j| levels[j] < levels[index]);
        assert_eq!(node.parent, expected, "Elternteil von {:?} folgt der Ebenenfolge {levels:?}", node.title);
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
        assert_eq!(currents, 1, "{:?}: genau eine aktuelle Fassung", node.title);
    }
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let doc = baruch_typst::parse(&exported).expect("Export ist zerlegbares Typst");
    let outline: Vec<(u8, String)> = doc.headings.iter().map(|h| (h.level, h.title.clone())).collect();
    let expected: Vec<(u8, String)> = loaded
        .nodes
        .iter()
        .zip(&levels)
        .filter_map(|(n, l)| {
            let body = current_body(&loaded, n.id);
            match *l {
                // Teile seit Runde 14 mit Teil-Markierung (Ebene 0 beim Zerlegen); die des
                // Beispielwerks tragen als Text noch „= Teil …“ (Altbestand, Ebene 1), leere gar nichts.
                0 if body.starts_with(&baruch_typst::part_line(&n.title)) => Some((0, n.title.clone())),
                0 if body.starts_with("= ") => Some((1, n.title.clone())),
                0 => None,
                level => Some((level, n.title.clone())),
            }
        })
        .collect();
    assert_eq!(outline, expected, "Export ergibt dieselbe Gliederung (Ebene, Titel) in neuer Reihenfolge");
    loaded
}

// ---- Migration 0006 auf einer Datenbank im Stand davor ---------------------------------------

async fn database_before_0006(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    let old: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| m.version < 6).cloned().collect();
    assert!(old.iter().any(|m| m.version == 4), "0001–0004 vorhanden");
    let migrator = sqlx::migrate::Migrator {
        migrations: Cow::Owned(old),
        ignore_missing: true,
        ..sqlx::migrate::Migrator::DEFAULT
    };
    migrator.run(&pool).await.unwrap();
    pool
}

/// Beispielwerk wie die alte Saat (mit Teilen, Vorwort ohne Nummer, Beiträgen und Verweisen).
async fn insert_sample(pool: &SqlitePool) {
    let work = sample::work();
    let work_id: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author) VALUES ('beispiel', 'book', ?, ?) RETURNING id",
    )
    .bind(&work.title)
    .bind(&work.author)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut positions = std::collections::HashMap::new();
    for node in &work.nodes {
        let position = positions.entry(node.parent).or_insert(0i64);
        let kind = match node.kind {
            baruch_core::NodeKind::Part => "part",
            baruch_core::NodeKind::Chapter => "chapter",
            baruch_core::NodeKind::Section => "section",
        };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(*position)
        .bind(&node.number)
        .bind(&node.title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        *position += 1;
        let mut shown: Option<u32> = None;
        for r in &node.revisions {
            let (author_kind, name, model) = match &r.author {
                baruch_core::Author::Human { name } => ("human", name.clone(), None),
                baruch_core::Author::Assistant { persona, model } => {
                    ("assistant", persona.clone(), Some(model.clone()))
                }
            };
            let status = match r.status {
                RevisionStatus::Proposed => "proposed",
                RevisionStatus::Current => "current",
                RevisionStatus::Rejected => "rejected",
                RevisionStatus::Superseded => "superseded",
            };
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(node_id)
            .bind(i64::from(r.number))
            .bind(&r.body)
            .bind(author_kind)
            .bind(name)
            .bind(model)
            .bind(status)
            .bind(&r.note)
            .bind(shown.map(i64::from))
            .execute(pool)
            .await
            .unwrap();
            if matches!(r.status, RevisionStatus::Current | RevisionStatus::Superseded) {
                shown = Some(r.number);
            }
        }
        for (position, post) in node.posts.iter().enumerate() {
            let post_id: i64 = sqlx::query_scalar(
                "INSERT INTO post (node_id, position, author, body) VALUES (?, ?, ?, ?) RETURNING id",
            )
            .bind(node_id)
            .bind(position as i64)
            .bind(&post.author)
            .bind(&post.body)
            .fetch_one(pool)
            .await
            .unwrap();
            for (i, r) in post.refs.iter().enumerate() {
                sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, ?, ?, ?, ?)")
                    .bind(post_id)
                    .bind(i as i64)
                    .bind(r.node as i64)
                    .bind(i64::from(r.rev))
                    .bind(&r.quote)
                    .execute(pool)
                    .await
                    .unwrap();
            }
        }
    }
}

/// Typst-Datei so, wie der Import vor Runde 4 sie ablegte: Titel wörtlich (mit Maskierungen),
/// Nummern „1“, „1.1“.
async fn insert_old_import(pool: &SqlitePool, source: &str, slug: &str) {
    let doc = baruch_typst::parse(source).unwrap();
    let work_id: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256) \
         VALUES (?, 'book', ?, 'Test', ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(doc.title.clone().unwrap())
    .bind(&doc.template)
    .bind(format!("{slug}.typ"))
    .bind(slug)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut stack: Vec<(u8, usize, String, usize)> = Vec::new();
    let mut roots = 0usize;
    for (seq, heading) in doc.headings.iter().enumerate() {
        while stack.last().is_some_and(|&(level, ..)| level >= heading.level) {
            stack.pop();
        }
        let (parent, position, number) = match stack.last_mut() {
            Some((_, parent, parent_number, children)) => {
                *children += 1;
                (Some(*parent), *children - 1, format!("{parent_number}.{children}"))
            }
            None => {
                roots += 1;
                (None, roots - 1, roots.to_string())
            }
        };
        let kind = if heading.level == 1 { "chapter" } else { "section" };
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(seq as i64)
        .bind(parent.map(|p| p as i64))
        .bind(position as i64)
        .bind(&number)
        .bind(&heading.raw_title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', 'current', 'Import')",
        )
        .bind(node_id)
        .bind(&heading.body)
        .execute(pool)
        .await
        .unwrap();
        stack.push((heading.level, seq, number, 0));
    }
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}"))).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn migration_0006_resolves_escaped_titles_and_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let pool = database_before_0006(dir.path()).await;
    insert_sample(&pool).await;
    insert_old_import(&pool, MASKED, "maskiert").await;
    let raw_title: String =
        sqlx::query_scalar("SELECT title FROM node WHERE work_id = 2 AND seq = 0").fetch_one(&pool).await.unwrap();
    assert_eq!(raw_title, "Anna\\'s Laube", "vorher steht der Titel wörtlich in der Datenbank");
    let mut before = Vec::new();
    for table in ["work", "node", "revision", "post", "post_ref"] {
        before.push(count(&pool, table).await);
    }
    let bodies_before: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, body FROM revision ORDER BY id").fetch_all(&pool).await.unwrap();
    pool.close().await;

    let store = Store::open(dir.path()).await.expect("0006 läuft auf dem Stand 0001–0004");
    let pool = store.pool();
    let mut after = Vec::new();
    for table in ["work", "node", "revision", "post", "post_ref"] {
        after.push(count(pool, table).await);
    }
    assert_eq!(after, before, "gleiche Anzahl Werke, Knoten, Revisionen, Beiträge, Verweise; keine neue Saat");
    let bodies_after: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, body FROM revision ORDER BY id").fetch_all(pool).await.unwrap();
    assert_eq!(bodies_after, bodies_before, "Revisionstexte bleiben byte-gleich (Überschriftszeilen wörtlich)");

    let masked = store.load_work(2).await.unwrap();
    assert_eq!(
        titles(&masked),
        ["Anna's Laube", "#1 und *Stern*", "Pfad C:\\Daten"],
        "Maskierungen in Titeln aufgelöst (\\' # * und \\\\)"
    );
    assert_eq!(numbers(&masked), ["I", "I.1", "II"], "Nummern nach #set heading(numbering: \"I.1\") der Vorlage");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(2).await.unwrap()).into_owned(),
        MASKED,
        "Export byte-gleich zur Quelle"
    );

    let example = store.load_work(1).await.unwrap();
    let vorwort: i64 =
        sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = 1 AND seq = 0").fetch_one(pool).await.unwrap();
    assert_eq!(vorwort, 0, "das Vorwort ohne Nummer zählt nach der Migration nicht mit");
    assert_eq!(
        numbers(&example),
        ["", "I", "1", "2", "2.1", "2.2", "II", "3", "3.1", "4", "4.1"],
        "Beispielwerk behält seine Nummern"
    );
    // Seit Runde 14 (Vertrag §4) projiziert der Export das Vorwort ohne Nummer als Markierung in der ersten
    // Überschriftszeile; die Revisionstexte bleiben unverändert (oben geprüft).
    let expected: String = sample::work()
        .nodes
        .iter()
        .map(|n| {
            let body = n.revisions[n.current_index().unwrap()].body.clone();
            if n.id == 0 { baruch_typst::set_numbered_line(&body, false).unwrap() } else { body }
        })
        .collect();
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(1).await.unwrap()).into_owned(),
        expected,
        "Export des Beispielwerks unverändert bis auf die Projektion"
    );
    let refs = &example.nodes[5].posts[1].refs;
    assert_eq!(
        refs,
        &vec![TextRef { node: 4, rev: 2, quote: "Langmut heißt nicht, Unrecht zu dulden.".into() }],
        "Verweise im Beitrag bleiben"
    );
    for node in &example.nodes {
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
        assert_eq!(currents, 1, "{}: genau eine aktuelle Fassung", node.title);
    }
}

#[tokio::test]
async fn a_fresh_database_numbers_the_example_and_an_import_like_before() {
    let (_dir, store) = fresh().await;
    let example = store.load_work(store.default_work().await.unwrap()).await.unwrap();
    assert_eq!(
        numbers(&example),
        ["", "I", "1", "2", "2.1", "2.2", "II", "3", "3.1", "4", "4.1"],
        "Teile römisch, Kapitel über Teile fortlaufend, Vorwort ohne Nummer (Trigger aus 0006)"
    );
    let garden = store.import_typst(GARDEN, "garten.typ", HUMAN).await.unwrap().work;
    let loaded = store.load_work(garden).await.unwrap();
    assert_eq!(numbers(&loaded), ["1", "1.1", "1.1.1", "2", "3"], "ohne Muster gilt 1.1");
    let roman = store.import_typst(MASKED, "maskiert.typ", HUMAN).await.unwrap().work;
    let loaded = store.load_work(roman).await.unwrap();
    assert_eq!(numbers(&loaded), ["I", "I.1", "II"], "Werkvorlage mit numbering: \"I.1\" zeigt römische Kapitel");
    assert_eq!(titles(&loaded)[0], "Anna's Laube", "der Import legt aufgelöste Titel ab");
}

#[tokio::test]
async fn the_numbering_can_be_set_switched_off_and_taken_from_the_template_again() {
    let (_dir, store) = fresh().await;
    let work = store.import_typst(MASKED, "maskiert.typ", HUMAN).await.unwrap().work;
    let done = store.set_numbering(work, Some("1.a")).await.unwrap();
    assert_eq!(done.action, OutlineAction::Numbering, "die Änderung ist eine Nummerierung");
    assert_eq!(numbers(&store.load_work(work).await.unwrap()), ["1", "1.a", "2"], "eigenes Muster gilt");
    store.set_numbering(work, Some("")).await.unwrap();
    assert_eq!(numbers(&store.load_work(work).await.unwrap()), ["", "", ""], "leeres Muster: keine Nummern");
    store.set_numbering(work, None).await.unwrap();
    assert_eq!(numbers(&store.load_work(work).await.unwrap()), ["I", "I.1", "II"], "wieder aus der Werkvorlage");
    let info = store.outline_numbering(work).await.unwrap();
    assert_eq!(
        (info.numbering, info.template.as_deref(), info.effective.as_str()),
        (None, Some("I.1"), "I.1"),
        "Muster aus der Vorlage gilt"
    );
    assert!(
        matches!(store.set_numbering(work, Some("Ohne")).await, Err(StoreError::Numbering(_))),
        "ein Muster ohne Zählzeichen wird abgelehnt (Runde 6: als Fehler der Nummerierung, RPC -32602)"
    );
    assert!(
        matches!(store.set_numbering(work, None).await, Err(StoreError::Refused(_))),
        "unverändert (schon aus der Werkvorlage) wird abgelehnt"
    );
    let stored: Vec<String> = sqlx::query_scalar("SELECT number FROM node WHERE work_id = ? ORDER BY seq")
        .bind(work)
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert_eq!(stored, ["I", "I.1", "II"], "node.number in der Datenbank ist mitgeschrieben");
}

// ---- Verschieben, Ebene, Umbenennen, Anlegen, Löschen ----------------------------------------

#[tokio::test]
async fn moving_a_chapter_renumbers_in_tree_order_and_keeps_the_texts() {
    let (_dir, store, work) = garden().await;
    let before = store.load_work(work).await.unwrap();
    let done = store.outline_move(work, 3, 0, Place::Before, HUMAN).await.unwrap();
    assert_eq!(done.action, OutlineAction::Moved, "die Änderung ist ein Verschieben");
    assert_eq!(done.node, 0, "Sommer steht jetzt an erster Stelle");
    assert_eq!(done.detail, "„Sommer“ vor „Frühling“ verschoben", "Betreff nennt beide Titel");
    assert_eq!(
        done.renumbered,
        vec![(3, 0), (0, 1), (1, 2), (2, 3)],
        "seq in Baumreihenfolge neu vergeben, [alt, neu]"
    );
    assert!(done.revisions.is_empty(), "gleiche Ebene: keine neue Fassung");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after), ["Sommer", "Frühling", "Aussaat", "Tiefe", "Herbst"], "neue Reihenfolge");
    assert_eq!(numbers(&after), ["1", "2", "2.1", "2.1.1", "3"], "Nummern neu berechnet");
    for (old, new) in [(3, 0), (0, 1), (1, 2), (2, 3), (4, 4)] {
        assert_eq!(current_body(&after, new), current_body(&before, old), "Text unverändert, nur die Stelle neu");
    }
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(exported.find("= Sommer").unwrap() < exported.find("= Frühling").unwrap(), "Export in neuer Reihenfolge");
}

#[tokio::test]
async fn moving_a_section_to_another_chapter_rewrites_only_heading_lines_and_post_refs() {
    let (_dir, store, work) = garden().await;
    // Beitrag in „Herbst“ mit Verweis auf „Tiefe“ (seq 2).
    let post = Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Siehe Tiefe.".into(),
        sources: vec![],
        refs: vec![TextRef { node: 2, rev: 1, quote: "Zwei Zentimeter.".into() }],
    };
    store.add_post(work, 4, &post).await.unwrap();
    // „Aussaat“ (Ebene 2, mit „Tiefe“) unter „Herbst“: bleibt Abschnitt, „Tiefe“ wandert mit.
    let done = store.outline_move(work, 1, 4, Place::Into, HUMAN).await.unwrap();
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after), ["Frühling", "Sommer", "Herbst", "Aussaat", "Tiefe"], "Teilbaum wandert mit");
    assert!(done.revisions.is_empty(), "gleiche Ebenen: kein Text ändert sich: {:?}", done.revisions);
    let herbst = &after.nodes[2];
    assert_eq!(herbst.posts[0].refs[0].node, 4, "post_ref zeigt auf die neue seq von „Tiefe“");
    assert_eq!(after.nodes[4].parent, Some(3), "Tiefe hängt weiter an Aussaat");
}

#[tokio::test]
async fn invalid_targets_are_refused_and_nothing_changes() {
    let (_dir, store, work) = garden().await;
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let revisions = count(store.pool(), "revision").await;
    let cases = [
        (3, 1, Place::Into, "Kapitel unter einen Abschnitt"),
        (1, 2, Place::Into, "in den eigenen Teilbaum (Tiefe liegt unter Aussaat)"),
        (0, 2, Place::After, "Kapitel hinter seinen eigenen Unterabschnitt"),
        (1, 0, Place::Before, "Abschnitt vor ein Kapitel würde ein Kapitel"),
        (1, 0, Place::Into, "steht schon an dieser Stelle"),
        (2, 3, Place::Into, "Unterabschnitt (Ebene 3) unter ein Kapitel würde Abschnitt (Ebene 2)"),
        (2, 0, Place::Before, "Unterabschnitt vor ein Kapitel würde Kapitel"),
        (3, 1, Place::After, "Kapitel hinter einen Abschnitt würde Abschnitt"),
    ];
    for (node, target, place, why) in &cases {
        let result = store.outline_move(work, *node, *target, *place, HUMAN).await;
        assert!(matches!(result, Err(StoreError::Refused(_))), "{why}: {result:?}");
    }
    // Ändert ein Zug die Ebene, nennt die Meldung den Weg über ← / →.
    for (node, target, place) in [(2, 3, Place::Into), (1, 0, Place::Before), (3, 1, Place::Into)] {
        let refused = store.outline_move(work, node, target, place, HUMAN).await.unwrap_err().to_string();
        assert_eq!(
            refused, "Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).",
            "Zug {node} {place:?} {target} änderte die Ebene, die Meldung sagt, wie es geht"
        );
    }
    let deep = store.outline_set_level(work, 1, 3, HUMAN).await;
    assert!(matches!(deep, Err(StoreError::Refused(_))), "Aussaat auf 3 schöbe Tiefe auf Ebene 4: {deep:?}");
    let four = store.outline_set_level(work, 2, 4, HUMAN).await;
    assert!(matches!(four, Err(StoreError::Refused(_))), "Ebene 4 gibt es nicht: {four:?}");
    let first = store.outline_set_level(work, 0, 2, HUMAN).await;
    assert!(matches!(first, Err(StoreError::Refused(_))), "das erste Kapitel kann nicht Abschnitt werden: {first:?}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        exported,
        "abgelehnte Züge ändern keinen Text"
    );
    assert_eq!(count(store.pool(), "revision").await, revisions, "und legen keine Fassung an");
    assert_consistent(&store, work).await;
    // Gegenprobe: Erst umstufen (Tiefe 3 → 2), dann geht derselbe Zug unter das andere Kapitel.
    store.outline_set_level(work, 2, 2, HUMAN).await.expect("Tiefe wird Abschnitt");
    let moved = store.outline_move(work, 2, 3, Place::Into, HUMAN).await;
    assert!(moved.is_ok(), "ein Abschnitt (Ebene 2) unter ein Kapitel behält die Ebene: {moved:?}");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after), ["Frühling", "Aussaat", "Sommer", "Tiefe", "Herbst"], "Tiefe steht unter Sommer");
    assert_eq!(
        current_body(&after, 3),
        "== Tiefe\n\nZwei Zentimeter.\n\n",
        "die Überschriftszeile kam vom Umstufen, das Verschieben ließ den Text"
    );
}

#[tokio::test]
async fn parts_never_change_their_level_and_a_point_beside_a_chapter_stays_a_chapter() {
    let (_dir, store) = fresh().await;
    let example = store.default_work().await.unwrap();
    // Teil I (seq 1) des Beispielwerks: kein Umstufen, kein Teil unter einem Kapitel.
    assert!(
        matches!(store.outline_set_level(example, 1, 1, HUMAN).await, Err(StoreError::Refused(_))),
        "Teil → Kapitel"
    );
    assert!(
        matches!(store.outline_set_level(example, 2, 0, HUMAN).await, Err(StoreError::Refused(_))),
        "Kapitel → Teil"
    );
    assert!(
        matches!(store.outline_move(example, 6, 2, Place::Into, HUMAN).await, Err(StoreError::Refused(_))),
        "ein Teil lässt sich nicht unter ein Kapitel hängen"
    );
    let added = store.outline_add(example, 6, Place::After, "Teil III – Leben", HUMAN).await.unwrap();
    let after = assert_consistent(&store, example).await;
    assert_eq!(after.nodes[added.node].kind, baruch_core::NodeKind::Part, "neben einem Teil entsteht ein Teil");
    assert_eq!(after.nodes[added.node].number, "III", "Teile zählen römisch");
    // Ohne Ebene gilt die Ebene des Orts: neben einem Kapitel entsteht ein Kapitel, kein Teil. (Bis
    // Runde 13 hieß es hier „mit Werkvorlage gibt es keine Teile“; seit Runde 14 legt Ebene 0 auch dort
    // einen Teil an – siehe round14_outline.rs.)
    let masked = store.import_typst(MASKED, "maskiert.typ", HUMAN).await.unwrap().work;
    let none = store.outline_add(masked, 0, Place::After, "Kapitel", HUMAN).await.unwrap();
    assert_eq!(
        store.load_work(masked).await.unwrap().nodes[none.node].kind,
        baruch_core::NodeKind::Chapter,
        "neben einem Kapitel entsteht ein Kapitel"
    );
}

#[tokio::test]
async fn set_level_moves_the_subnodes_along_and_writes_new_heading_lines() {
    let (_dir, store, work) = garden().await;
    let done = store.outline_set_level(work, 1, 1, HUMAN).await.unwrap();
    assert_eq!(done.detail, "„Aussaat“ Ebene 2 → 1", "Betreff mit alter und neuer Ebene");
    let after = assert_consistent(&store, work).await;
    assert_eq!(levels(&after), [1, 1, 2, 1, 1], "Aussaat wird Kapitel, Tiefe wandert mit (3 → 2)");
    assert_eq!(numbers(&after), ["1", "2", "2.1", "3", "4"], "Nummern nach dem Umstufen");
    assert_eq!(current_body(&after, 1), "= Aussaat <aussaat>\n\nRadieschen.\n\n", "nur die Zeile, Label bleibt");
    assert_eq!(current_body(&after, 2), "== Tiefe\n\nZwei Zentimeter.\n\n", "Unterpunkt: neue Überschriftszeile");
    assert_eq!(done.revisions, vec![(1, 2), (2, 2)], "je betroffenem Knoten eine neue Fassung");
    let node = &after.nodes[1];
    let rev = node.revision(2).unwrap();
    assert!(matches!(&rev.author, baruch_core::Author::Human { name } if name == HUMAN), "Fassung vom Menschen");
    assert_eq!(node.revision(1).unwrap().status, RevisionStatus::Superseded, "die alte bleibt als ersetzt");
}

#[tokio::test]
async fn renaming_writes_an_escaped_heading_line_and_the_resolved_title() {
    let (_dir, store, work) = garden().await;
    let done = store.outline_rename(work, 1, "Saat & *Pflege*", HUMAN).await.unwrap();
    assert_eq!(done.detail, "„Aussaat“ → „Saat & *Pflege*“ umbenannt", "Betreff nennt alten und neuen Titel");
    let after = assert_consistent(&store, work).await;
    assert_eq!(after.nodes[1].title, "Saat & *Pflege*", "Titel aufgelöst");
    assert_eq!(current_body(&after, 1), "== Saat & \\*Pflege\\* <aussaat>\n\nRadieschen.\n\n", "Zeile maskiert");
    assert!(
        matches!(store.outline_rename(work, 1, "  ", HUMAN).await, Err(StoreError::Title(_))),
        "leerer Titel abgelehnt (Runde 7: StoreError::Title)"
    );
}

#[tokio::test]
async fn adding_before_after_and_into_places_new_points_with_a_human_first_version() {
    let (_dir, store, work) = garden().await;
    let into = store.outline_add(work, 3, Place::Into, "Gießplan", HUMAN).await.unwrap();
    let after = assert_consistent(&store, work).await;
    assert_eq!(into.node, 4, "letzter Unterpunkt von Sommer");
    assert_eq!(after.nodes[4].parent, Some(3), "der neue Punkt hängt unter Sommer");
    assert_eq!(current_body(&after, 4), "== Gießplan\n\n", "Revision 1 = Überschriftszeile + Leerzeile");
    let before = store.outline_add(work, 0, Place::Before, "Vorwort", HUMAN).await.unwrap();
    assert_eq!(before.node, 0, "der neue Punkt steht vorn");
    let after = assert_consistent(&store, work).await;
    assert_eq!(numbers(&after)[..2], ["1", "2"], "neues erstes Kapitel zählt mit");
    let behind = store.outline_add(work, 2, Place::After, "Pflanzabstand", HUMAN).await.unwrap();
    let after = assert_consistent(&store, work).await;
    assert_eq!(after.nodes[behind.node].title, "Pflanzabstand", "der neue Punkt trägt seinen Titel");
    assert_eq!(after.nodes[behind.node].parent, Some(1), "hinter Aussaat samt Tiefe, unter Frühling");
}

#[tokio::test]
async fn deleting_keeps_the_text_for_the_mirror_and_refuses_points_with_children() {
    let (_dir, store, work) = garden().await;
    let refused = store.outline_delete(work, 1, HUMAN).await;
    assert!(
        matches!(refused, Err(StoreError::Refused(m)) if m.contains("Unterpunkte")),
        "mit Unterpunkten: {refused:?}"
    );
    // Verweis aus „Sommer“ auf „Tiefe“ (seq 2) entfällt mit dem Knoten.
    let post = Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "Notiz".into(),
        sources: vec![],
        refs: vec![
            TextRef { node: 2, rev: 1, quote: "Zwei".into() },
            TextRef { node: 4, rev: 1, quote: "Ernte".into() },
        ],
    };
    store.add_post(work, 3, &post).await.unwrap();
    store.add_post(work, 2, &Post { refs: vec![], ..post.clone() }).await.unwrap();
    let done = store.outline_delete(work, 2, HUMAN).await.unwrap();
    assert_eq!(done.detail, "„Tiefe“ gelöscht", "Betreff nennt den gelöschten Punkt");
    assert_eq!(
        done.removed,
        vec![("Tiefe".to_owned(), "=== Tiefe\n\nZwei Zentimeter.\n\n".to_owned())],
        "der gelöschte Text bleibt für den Spiegel"
    );
    assert_eq!(done.node, 1, "danach gezeigt: der vorige Punkt");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after), ["Frühling", "Aussaat", "Sommer", "Herbst"], "Tiefe fehlt danach");
    assert_eq!(
        after.nodes[2].posts[0].refs,
        vec![TextRef { node: 3, rev: 1, quote: "Ernte".into() }],
        "Verweis auf den gelöschten Knoten entfällt, der andere zeigt auf die neue seq"
    );
    let orphans: i64 = sqlx::query_scalar("SELECT count(*) FROM revision WHERE node_id NOT IN (SELECT id FROM node)")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(orphans, 0, "keine Revisionen ohne Knoten");
    // Der Trigger schützt weiter lebende Abschnitte.
    let protect = sqlx::query("DELETE FROM revision WHERE status = 'current'").execute(store.pool()).await;
    assert!(protect.is_err(), "aktuelle Fassungen lebender Abschnitte bleiben unlöschbar");
    for seq in [3, 2, 1] {
        store.outline_delete(work, seq, HUMAN).await.unwrap();
    }
    let last = store.outline_delete(work, 0, HUMAN).await;
    assert!(matches!(last, Err(StoreError::Refused(_))), "das letzte Kapitel bleibt: {last:?}");
}

// ---- Überschriften im Text -------------------------------------------------------------------

#[tokio::test]
async fn checking_a_text_reports_split_and_merge_without_writing() {
    let (_dir, store, work) = garden().await;
    let before = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let split = store.outline_check(work, 3, "= Sommer\n\nGießen.\n\n== Mulchen\n\nStroh.\n").await.unwrap();
    assert_eq!(split.split, vec!["Mulchen".to_owned()], "neue Überschrift erkannt");
    assert!(split.valid && !split.merge, "Teilen wäre gültig");
    let merge = store.outline_check(work, 3, "Gießen ohne Überschrift.\n").await.unwrap();
    assert!(merge.merge && merge.valid, "keine Überschrift: Zusammenführen mit Aussaat/Tiefe");
    let first = store.outline_check(work, 0, "Ohne Überschrift.\n").await.unwrap();
    assert!(first.merge && !first.valid, "der erste Punkt lässt sich nicht zusammenführen");
    let deep = store.outline_check(work, 0, "=== Zu tief\n").await.unwrap();
    assert!(!deep.valid && deep.message.is_some(), "der erste Punkt muss ein Kapitel bleiben: {deep:?}");
    let level = store.outline_check(work, 1, "= Aussaat\n").await.unwrap();
    assert_eq!((level.level, level.title.as_deref()), (Some(1), Some("Aussaat")), "Ebene und Titel aus der Zeile");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        before,
        "Prüfen schreibt nichts"
    );
}

#[tokio::test]
async fn saving_a_changed_heading_line_adapts_title_and_level() {
    let (_dir, store, work) = garden().await;
    let saved = store
        .outline_save(work, 1, "= Aussaat neu\n\nRadieschen.\n\n", Some(1), Restructure::default(), HUMAN)
        .await
        .unwrap();
    let outline = saved.outline.expect("Gliederung geändert");
    assert_eq!(outline.action, OutlineAction::Level, "Ebenenwechsel");
    assert!(saved.change.is_some(), "neue Fassung gespeichert");
    let after = assert_consistent(&store, work).await;
    assert_eq!(after.nodes[1].title, "Aussaat neu", "Titel aus der Zeile");
    assert_eq!(levels(&after), [1, 1, 2, 1, 1], "Aussaat wird Kapitel, Tiefe wandert mit (Ebene 3 → 2)");
    assert_eq!(current_body(&after, 2), "== Tiefe\n\nZwei Zentimeter.\n\n", "Unterpunkt bekommt die neue Zeile");
    assert_eq!(outline.revisions, vec![(1, 2), (2, 2)], "gespeicherte Fassung und neue Zeile des Unterpunkts");
    // Nur der Titel: Umbenennen ohne Umstellen.
    let renamed =
        store.outline_save(work, 3, "= Hochsommer\n\nGießen.\n\n", None, Restructure::default(), HUMAN).await.unwrap();
    assert_eq!(renamed.outline.unwrap().action, OutlineAction::Renamed, "nur der Titel: Umbenennen");
    assert_eq!(
        store.load_work(work).await.unwrap().nodes[3].title,
        "Hochsommer",
        "der Punkt heißt wie die Überschrift"
    );
    // Reiner Text: keine Gliederungsänderung.
    let text =
        store.outline_save(work, 4, "= Herbst\n\nErnte, viel.\n", None, Restructure::default(), HUMAN).await.unwrap();
    assert!(text.outline.is_none() && text.change.is_some(), "nur der Text ändert sich");
}

#[tokio::test]
async fn an_extra_heading_splits_only_after_confirmation() {
    let (_dir, store, work) = garden().await;
    let body = "= Sommer\n\nGießen.\n\n== Mulchen\n\nStroh.\n\n== Ernten\n\nBohnen.\n";
    let refused = store.outline_save(work, 3, body, None, Restructure::default(), HUMAN).await;
    assert!(
        matches!(refused, Err(StoreError::Refused(m)) if m.contains("Teilen")),
        "ohne split abgelehnt: {refused:?}"
    );
    assert_eq!(store.load_work(work).await.unwrap().nodes.len(), 5, "nichts geändert");
    let saved =
        store.outline_save(work, 3, body, None, Restructure { split: true, merge: false }, HUMAN).await.unwrap();
    let outline = saved.outline.unwrap();
    assert_eq!(outline.action, OutlineAction::Split, "weitere Überschriften teilen");
    assert_eq!(outline.detail, "„Sommer“ geteilt: „Mulchen“, „Ernten“ neu", "Betreff nennt die neuen Punkte");
    let after = assert_consistent(&store, work).await;
    assert_eq!(
        titles(&after),
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Mulchen", "Ernten", "Herbst"],
        "die neuen Punkte stehen hinter Sommer"
    );
    assert_eq!(current_body(&after, 3), "= Sommer\n\nGießen.\n\n", "der erste Teil bleibt im Knoten");
    assert_eq!(current_body(&after, 4), "== Mulchen\n\nStroh.\n\n", "neuer Knoten mit seinem Stück");
    assert_eq!(after.nodes[5].revisions.len(), 1, "neue Knoten haben Revision 1 vom Menschen");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        GARDEN.replace("= Sommer\n\nGießen.\n\n", body),
        "Export = neuer Text"
    );
}

#[tokio::test]
async fn a_removed_heading_merges_with_the_previous_point_only_after_confirmation() {
    let (_dir, store, work) = garden().await;
    let post = Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Zum Gießen".into(),
        sources: vec![],
        refs: vec![],
    };
    store.add_post(work, 3, &post).await.unwrap();
    store.add_post(work, 2, &Post { body: "Zur Tiefe".into(), ..post.clone() }).await.unwrap();
    let refused = store.outline_save(work, 3, "Gießen.\n", None, Restructure::default(), HUMAN).await;
    assert!(matches!(refused, Err(StoreError::Refused(_))), "ohne merge abgelehnt");
    let saved =
        store.outline_save(work, 3, "Gießen.\n", None, Restructure { split: false, merge: true }, HUMAN).await.unwrap();
    let outline = saved.outline.unwrap();
    assert_eq!(outline.action, OutlineAction::Merged, "ohne Überschrift wird zusammengeführt");
    assert_eq!(outline.node, 2, "gezeigt wird Tiefe, an die angehängt wurde");
    assert_eq!(outline.removed[0].1, "= Sommer\n\nGießen.\n\n", "der alte Text steht für den Spiegel bereit");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after), ["Frühling", "Aussaat", "Tiefe", "Herbst"], "Sommer ist im vorigen Punkt aufgegangen");
    assert_eq!(current_body(&after, 2), "=== Tiefe\n\nZwei Zentimeter.\n\nGießen.\n", "Text angehängt");
    let bodies: Vec<&str> = after.nodes[2].posts.iter().map(|p| p.body.as_str()).collect();
    assert_eq!(bodies, ["Zur Tiefe", "Zum Gießen"], "Beiträge wandern mit, hinter die vorhandenen");
    let first =
        store.outline_save(work, 0, "Nur Text.\n", None, Restructure { split: false, merge: true }, HUMAN).await;
    assert!(matches!(first, Err(StoreError::Refused(_))), "erster Knoten nicht zusammenführbar");
}

#[tokio::test]
async fn without_any_change_the_export_stays_byte_for_byte_the_source() {
    let (_dir, store, work) = garden().await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        GARDEN,
        "frisch importiert: byte-gleich"
    );
    let saved = store
        .outline_save(
            work,
            0,
            &GARDEN[GARDEN.find("= Frühling").unwrap()..GARDEN.find("== Aussaat").unwrap()],
            None,
            Restructure::default(),
            HUMAN,
        )
        .await
        .unwrap();
    assert!(saved.change.is_none() && saved.outline.is_none(), "gleicher Text: nichts gespeichert");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        GARDEN,
        "weiter byte-gleich"
    );
}

// ---- Überschriften außerhalb des Editors und feste Schlüssel ------------------------------------

#[tokio::test]
async fn accepting_an_old_proposal_with_other_headings_is_refused_and_rolled_back() {
    let (_dir, store, work) = garden().await;
    // Ein Vorschlag aus der Zeit vor der Prüfung (direkt in der Datenbank): Sommer als Abschnitt
    // und ein neues Kapitel „Winter“.
    let sommer = work_node_id(&store, work, 3).await;
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, 2, '== Sommer\n\nC.\n\n= Winter\n\nD.\n', 'assistant', '@lektor', 'm', 'proposed', '', 1)",
    )
    .bind(sommer)
    .execute(store.pool())
    .await
    .unwrap();
    let refused = store.accept(work, 3, 2).await.unwrap_err();
    assert!(
        matches!(&refused, StoreError::Heading(m) if m.contains("„Winter“")),
        "Übernehmen abgelehnt, die Meldung nennt die weitere Überschrift: {refused:?}"
    );
    let loaded = assert_consistent(&store, work).await;
    assert_eq!(current_body(&loaded, 3), "= Sommer\n\nGießen.\n\n", "Fassung 1 bleibt aktuell (zurückgerollt)");
    assert_eq!(titles(&loaded), ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst"], "Gliederung unverändert");
    let again = store.add_proposal(work, 3, "== Sommer\n\nC.\n", "@lektor", "m", "").await.unwrap_err();
    assert!(matches!(again, StoreError::Heading(_)), "ein neuer Vorschlag mit anderer Ebene wird nicht angelegt");
}

#[tokio::test]
async fn the_key_of_a_point_survives_every_outline_change_until_it_is_deleted() {
    let (_dir, store, work) = garden().await;
    let before = store.load_work(work).await.unwrap();
    let summer = before.nodes[3].key;
    assert_eq!(store.node_seq(work, summer).await.unwrap(), 3, "der Schlüssel führt zur Stelle");
    store.outline_move(work, 3, 0, Place::Before, HUMAN).await.unwrap();
    assert_eq!(store.node_seq(work, summer).await.unwrap(), 0, "nach dem Verschieben die neue Stelle");
    store.outline_add(work, 0, Place::Before, "Vorfrühling", HUMAN).await.unwrap();
    assert_eq!(store.node_seq(work, summer).await.unwrap(), 1, "ein neuer Punkt davor schiebt die Stelle weiter");
    let after = store.load_work(work).await.unwrap();
    assert_eq!(after.nodes[1].title, "Sommer", "an der neuen Stelle steht derselbe Punkt");
    assert_eq!(after.nodes[1].key, summer, "und er trägt denselben Schlüssel");
    store.outline_delete(work, 1, HUMAN).await.unwrap();
    let gone = store.node_seq(work, summer).await.unwrap_err();
    assert!(matches!(gone, StoreError::GoneNode { key, .. } if key == summer), "gelöscht → GoneNode: {gone:?}");
    let foreign = store.node_seq(work + 100, summer).await.unwrap_err();
    assert!(matches!(foreign, StoreError::UnknownWork(_)), "fremdes Werk → UnknownWork: {foreign:?}");
}

async fn work_node_id(store: &Store, work: WorkId, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn the_numbering_label_names_the_matching_scheme_instead_of_the_raw_pattern() {
    // Runde 7: Der Gliederungskopf zeigte „Nummern: I|1“ roh.
    let (_dir, store, work) = garden().await;
    let label = |info: baruch_store::NumberingInfo| info.label;
    assert_eq!(
        label(store.outline_numbering(work).await.unwrap()),
        "Dezimal (DIN 1421)",
        "aus der Werkvorlage ohne Regel gilt 1.1 = Dezimal"
    );
    for (numbering, expected) in [
        (Some("I|1"), "Nur Kapitel römisch"),
        (Some("1.a)"), "1.a)"),
        (Some(""), "keine"),
        (Some("I.;A.;1.;a);aa)"), "Alphanumerisch"),
    ] {
        store.set_numbering(work, numbering).await.unwrap();
        let info = store.outline_numbering(work).await.unwrap();
        assert_eq!(info.label, expected, "{numbering:?}: Name des passenden Schemas, sonst das Muster: {info:?}");
    }
    store.set_numbering_scheme(work, "legal", None).await.unwrap();
    assert_eq!(label(store.outline_numbering(work).await.unwrap()), "Juristisch", "Schema mit Wörtern: sein Name");
    store.set_numbering_scheme(work, "chapter_roman", None).await.unwrap();
    assert_eq!(label(store.outline_numbering(work).await.unwrap()), "Nur Kapitel römisch", "das Schema selbst");

    let roman = store.import_typst(MASKED, "maskiert.typ", HUMAN).await.unwrap().work;
    let info = store.outline_numbering(roman).await.unwrap();
    assert_eq!(
        (info.scheme.as_str(), info.label.as_str()),
        ("template", "I.1"),
        "Vorlagenmuster ohne Schema: {info:?}"
    );
}
