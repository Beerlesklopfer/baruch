// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschalten bringt die Ebene der Fassung mit (Vertrag Runde 5, Abschnitte 4 und 6; Nutzerentscheidung
//! „Nein“: eine Fassung wird beim Umschalten nie umgeschrieben, stattdessen folgt die Gliederung).
//! Echte SQLite-Datei im Temp-Ordner, eigenes kleines Buch (frei erfunden, unten).

use baruch_core::{RevisionStatus, Work};
use baruch_store::{ChangeKind, OutlineAction, Place, RevisionEdit, Store, StoreError, WorkId};

/// Eigenes kleines Buch ohne Nummerierungsregel: Kapitel, Abschnitt mit Label, Unterabschnitt.
const GARDEN: &str = "\
#set document(title: \"Umschaltprobe\", author: \"Test\")

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

const HUMAN: &str = "Autor";

async fn garden() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(GARDEN, "umschalten.typ", HUMAN).await.unwrap().work;
    (dir, store, work)
}

fn titles(work: &Work) -> Vec<&str> {
    work.nodes.iter().map(|n| n.title.as_str()).collect()
}

/// Ebene je Knoten aus Art und Elternteil (wie der Store sie sieht).
fn levels(work: &Work) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for node in &work.nodes {
        out.push(match node.kind {
            baruch_core::NodeKind::Part => 0,
            baruch_core::NodeKind::Chapter => 1,
            baruch_core::NodeKind::Section => match node.parent.map(|p| out[p]) {
                Some(p) if p >= 2 => 3,
                _ => 2,
            },
        });
    }
    out
}

fn body(work: &Work, node: usize, rev: u32) -> &str {
    &work.nodes[node].revision(rev).expect("Fassung vorhanden").body
}

fn current(work: &Work, node: usize) -> u32 {
    work.nodes[node].current_number().expect("aktuelle Fassung")
}

async fn revisions(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM revision").fetch_one(store.pool()).await.unwrap()
}

/// Nach jeder Änderung: genau eine aktuelle Fassung je Abschnitt, und der Export ergibt beim Zerlegen
/// dieselbe Gliederung (Ebene, Titel) wie die Datenbank.
async fn assert_consistent(store: &Store, work: WorkId) -> Work {
    let loaded = store.load_work(work).await.expect("Werk lädt (seq lückenlos, genau eine current)");
    for node in &loaded.nodes {
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
        assert_eq!(currents, 1, "{:?}: genau eine aktuelle Fassung", node.title);
    }
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let doc = baruch_typst::parse(&exported).expect("Export ist zerlegbares Typst");
    let outline: Vec<(u8, String)> = doc.headings.iter().map(|h| (h.level, h.title.clone())).collect();
    let expected: Vec<(u8, String)> =
        loaded.nodes.iter().zip(levels(&loaded)).map(|(n, l)| (l, n.title.clone())).collect();
    assert_eq!(outline, expected, "Export und Gliederung zeigen dieselben Ebenen und Titel");
    loaded
}

/// Nur die erste Zeile unterscheidet sich (die Überschriftszeile).
fn only_heading_line_differs(before: &str, after: &str) -> bool {
    let (b_head, b_rest) = before.split_once('\n').unwrap_or((before, ""));
    let (a_head, a_rest) = after.split_once('\n').unwrap_or((after, ""));
    b_head != a_head && b_rest == a_rest
}

#[tokio::test]
async fn switching_to_a_version_with_another_level_moves_the_point_and_its_subpoints() {
    let (_dir, store, work) = garden().await;
    // Aussaat wird Kapitel (neue Fassung 2 „= Aussaat <aussaat>“), Tiefe dabei Abschnitt (Fassung 2).
    store.outline_set_level(work, 1, 1, HUMAN).await.unwrap();
    let before = assert_consistent(&store, work).await;
    assert_eq!(levels(&before), [1, 1, 2, 1, 1], "Ausgangslage: Aussaat Kapitel, Tiefe Abschnitt");
    let old_text = body(&before, 1, 1).to_owned();
    let tiefe_before = body(&before, 2, current(&before, 2)).to_owned();
    let count = revisions(&store).await;

    // Zurück auf Fassung 1 („== Aussaat <aussaat>“): Die Gliederung folgt ihr.
    let switch = store.switch_to(work, 1, 1, false, HUMAN).await.expect("Umschalten auf andere Ebene ist erlaubt");
    assert_eq!(switch.change.kind, ChangeKind::Switched, "eine ersetzte Fassung wird umgeschaltet, nicht übernommen");
    assert_eq!((switch.change.node, switch.change.rev), (1, 1), "die Änderung nennt Punkt und Fassung");
    let outline = switch.outline.expect("andere Ebene: die Gliederung hat sich geändert");
    assert_eq!(outline.action, OutlineAction::Level, "die Ebenenänderung ist ein OutlineChange mit Level");
    assert_eq!(outline.detail, "„Aussaat“ Ebene 1 → 2", "der Betreff nennt alte und neue Ebene");
    assert_eq!(outline.node, 1, "der Punkt behält seine Stelle");

    let after = assert_consistent(&store, work).await;
    assert_eq!(levels(&after), [1, 2, 3, 1, 1], "Aussaat wieder Abschnitt, Tiefe wieder Unterabschnitt");
    assert_eq!(titles(&after), ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst"], "Titel und Reihenfolge bleiben");
    assert_eq!(current(&after, 1), 1, "Fassung 1 ist aktuell");
    assert_eq!(body(&after, 1, 1), old_text, "der Text der Fassung bleibt byte-gleich (nie umgeschrieben)");
    assert_eq!(after.nodes[1].revisions.len(), 2, "für den umgeschalteten Punkt entsteht keine neue Fassung");
    let tiefe = current(&after, 2);
    assert_eq!(outline.revisions, vec![(2, tiefe)], "nur der mitgestufte Unterpunkt bekommt eine neue Fassung");
    let tiefe_now = body(&after, 2, tiefe);
    assert!(
        only_heading_line_differs(&tiefe_before, tiefe_now),
        "die neue Fassung von Tiefe unterscheidet sich nur in der Überschriftszeile: {tiefe_before:?} → {tiefe_now:?}"
    );
    assert!(tiefe_now.starts_with("=== Tiefe\n"), "Tiefe trägt wieder Ebene 3: {tiefe_now:?}");
    assert!(
        matches!(&after.nodes[2].revisions.last().unwrap().author, baruch_core::Author::Human { name } if name == HUMAN),
        "die neue Fassung des Unterpunkts stammt vom Menschen"
    );
    assert_eq!(revisions(&store).await, count + 1, "genau eine neue Fassung (die des Unterpunkts)");
}

#[tokio::test]
async fn the_title_follows_the_version_together_with_the_level() {
    let (_dir, store, work) = garden().await;
    // Im Editor: Aussaat wird Kapitel „Saat“ (Fassung 2), Tiefe dabei Abschnitt.
    let saved = store
        .outline_save(work, 1, "= Saat <aussaat>\n\nRadieschen.\n\n", None, Default::default(), HUMAN)
        .await
        .unwrap();
    assert!(saved.outline.is_some(), "Speichern hat die Gliederung angepasst");
    let switch = store.switch_to(work, 1, 1, false, HUMAN).await.unwrap();
    let outline = switch.outline.expect("andere Ebene");
    assert_eq!(outline.detail, "„Saat“ → „Aussaat“ umbenannt, Ebene 1 → 2", "Titel und Ebene folgen");
    assert_eq!(outline.title, "Aussaat", "die Änderung trägt den neuen Titel");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after)[1], "Aussaat", "der Punkt heißt wie die Überschrift der Fassung");
    assert_eq!(levels(&after), [1, 2, 3, 1, 1], "Ebenen wie im Import");
}

#[tokio::test]
async fn a_switch_that_would_push_subpoints_below_level_three_is_refused() {
    let (_dir, store, work) = garden().await;
    // Sommer bekommt Wasser (2) und darunter Kanne (3).
    let wasser = store.outline_add(work, 3, Place::Into, "Wasser", HUMAN).await.unwrap().node;
    store.outline_add(work, wasser, Place::Into, "Kanne", HUMAN).await.unwrap();
    // Eine neue aktuelle Fassung von Sommer; die alte Fassung 1 wird (nicht aktuell, ohne Prüfung) zu
    // einer Abschnittsüberschrift geändert.
    store.save_human(work, 3, "= Sommer\n\nGießen, neu.\n", HUMAN).await.unwrap().expect("neue Fassung");
    let edit = RevisionEdit { body: Some("== Sommer\n\nGießen.\n\n"), note: None };
    store.update_revision(work, 3, 1, &edit, HUMAN).await.unwrap().expect("geändert");
    let before = assert_consistent(&store, work).await;
    assert_eq!(levels(&before), [1, 2, 3, 1, 2, 3, 1], "Ausgangslage: Sommer › Wasser › Kanne");
    let sommer_current = current(&before, 3);

    // Kanne käme auf Ebene 4. Kappen auf 3 machte Kanne zum Geschwister von Wasser, und Zurückschalten
    // stellte das nicht wieder her (Befund nach Runde 5) – darum wird abgelehnt wie bei „Ebene ändern“.
    let refused = store.switch_to(work, 3, 1, false, HUMAN).await;
    let Err(StoreError::Heading(message)) = &refused else {
        panic!("Umschalten muss mit einer Überschriftsmeldung abgelehnt werden: {refused:?}");
    };
    assert!(message.contains("„Kanne“"), "die Meldung nennt den Unterpunkt, der zu tief käme: {message}");
    assert!(message.contains("höher stufen"), "die Meldung sagt, was zu tun ist: {message}");
    let after = assert_consistent(&store, work).await;
    assert_eq!(levels(&after), levels(&before), "die Gliederung bleibt, wie sie war");
    assert_eq!(current(&after, 3), sommer_current, "auch das Umschalten selbst ist zurückgerollt");
    for node in 4..=5 {
        assert_eq!(
            after.nodes[node].revisions.len(),
            before.nodes[node].revisions.len(),
            "Knoten {node} bekommt keine neue Fassung, weil nichts geschieht"
        );
    }
}

#[tokio::test]
async fn switching_there_and_back_restores_the_outline() {
    let (_dir, store, work) = garden().await;
    // Sommer bekommt nur Wasser (2); als Abschnitt rutscht Wasser auf 3, das geht.
    store.outline_add(work, 3, Place::Into, "Wasser", HUMAN).await.unwrap();
    store.save_human(work, 3, "= Sommer\n\nGießen, neu.\n", HUMAN).await.unwrap().expect("neue Fassung");
    let edit = RevisionEdit { body: Some("== Sommer\n\nGießen.\n\n"), note: None };
    store.update_revision(work, 3, 1, &edit, HUMAN).await.unwrap().expect("geändert");
    let before = assert_consistent(&store, work).await;
    assert_eq!(levels(&before), [1, 2, 3, 1, 2, 1], "Ausgangslage: Sommer › Wasser");
    let sommer_new = current(&before, 3);

    let there = store.switch_to(work, 3, 1, false, HUMAN).await.expect("Sommer wird Abschnitt");
    assert!(there.outline.is_some(), "die Ebene ändert sich");
    let middle = assert_consistent(&store, work).await;
    assert_eq!(levels(&middle), [1, 2, 3, 2, 3, 1], "Wasser wandert mit und bleibt unter Sommer");
    assert_eq!(middle.nodes[4].parent, Some(3), "Wasser bleibt Kind von Sommer");

    store.switch_to(work, 3, sommer_new, false, HUMAN).await.expect("zurück auf die Kapitelfassung");
    let back = assert_consistent(&store, work).await;
    assert_eq!(levels(&back), levels(&before), "Zurückschalten stellt die Ebenen wieder her");
    assert_eq!(back.nodes[4].parent, before.nodes[4].parent, "und die Eltern-Kind-Beziehung");
    assert!(
        body(&back, 4, current(&back, 4)).starts_with("== Wasser\n"),
        "Wassers Überschriftszeile trägt wieder Ebene 2"
    );
}

#[tokio::test]
async fn accepting_a_proposal_with_another_level_moves_the_outline_too() {
    let (_dir, store, work) = garden().await;
    let proposal = store.add_proposal(work, 3, "= Sommer\n\nVorschlag.\n", "@lektor", "probe", "").await.unwrap();
    // Der Mensch ändert den Vorschlag (nicht aktuell, keine Prüfung) auf eine Abschnittsüberschrift.
    let edit = RevisionEdit { body: Some("== Sommer\n\nVorschlag.\n"), note: None };
    store.update_revision(work, 3, proposal, &edit, HUMAN).await.unwrap();
    let switch = store.switch_to(work, 3, proposal, true, HUMAN).await.expect("Übernehmen folgt der Ebene");
    assert_eq!(switch.change.kind, ChangeKind::Accepted, "ein Vorschlag wird übernommen");
    assert!(switch.outline.is_some(), "die Gliederung folgt auch beim Übernehmen");
    let after = assert_consistent(&store, work).await;
    assert_eq!(levels(&after), [1, 2, 3, 2, 1], "Sommer ist jetzt Abschnitt unter Frühling");
    assert_eq!(body(&after, 3, proposal), "== Sommer\n\nVorschlag.\n", "der Vorschlag bleibt wörtlich");
    // Die Hülle accept schaltet nur Vorschläge um.
    let refused = store.accept(work, 3, 1).await;
    assert!(matches!(refused, Err(StoreError::Refused(_))), "Fassung 1 ist kein Vorschlag: {refused:?}");
}

#[tokio::test]
async fn the_same_level_only_renames_and_reports_no_outline_change() {
    let (_dir, store, work) = garden().await;
    store.save_human(work, 3, "= Hochsommer\n\nHeiß.\n", HUMAN).await.unwrap();
    let switch = store.switch_to(work, 3, 1, false, HUMAN).await.unwrap();
    assert!(switch.outline.is_none(), "gleiche Ebene: keine Gliederungsänderung im Sinn von Level");
    let after = assert_consistent(&store, work).await;
    assert_eq!(titles(&after)[3], "Sommer", "der Titel folgt der Fassung wie bisher");
    // Die Hülle set_current liefert dieselbe Änderung wie switch_to.
    let change = store.set_current(work, 3, 2).await.unwrap();
    assert_eq!((change.kind, change.rev), (ChangeKind::Switched, 2), "set_current ist eine Hülle um switch_to");
    assert_eq!(titles(&store.load_work(work).await.unwrap())[3], "Hochsommer", "zurück heißt er wieder so");
}

#[tokio::test]
async fn versions_that_do_not_fit_are_refused_and_nothing_changes() {
    let (_dir, store, work) = garden().await;
    // Je Fall eine neue aktuelle Fassung von Sommer bzw. Frühling, dann die alte (nicht aktuell) ändern.
    store.save_human(work, 3, "= Sommer\n\nNeu.\n", HUMAN).await.unwrap();
    store.save_human(work, 0, "= Frühling\n\nNeu.\n", HUMAN).await.unwrap();
    let cases: [(usize, &str, &str); 4] = [
        (3, "Nur Text ohne Überschrift.\n", "keine Überschrift"),
        (3, "= Sommer\n\nA.\n\n== Hitze\n\nB.\n", "Hitze"),
        (3, "Vorab.\n\n= Sommer\n\nA.\n", "Speichern"),
        (0, "== Frühling\n\nDie Beete.\n\n", "Ebene 2"),
    ];
    for (node, text, needle) in cases {
        let edit = RevisionEdit { body: Some(text), note: None };
        store.update_revision(work, node, 1, &edit, HUMAN).await.unwrap();
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
        let count = revisions(&store).await;
        let refused = store.switch_to(work, node, 1, false, HUMAN).await;
        let message = match &refused {
            Err(StoreError::Heading(message)) => message.clone(),
            other => panic!("{text:?} muss abgelehnt werden: {other:?}"),
        };
        assert!(message.contains(needle), "die Meldung nennt den Grund ({needle}): {message}");
        let after = assert_consistent(&store, work).await;
        assert_eq!(current(&after, node), 2, "{text:?}: Fassung 2 bleibt aktuell (zurückgerollt)");
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
            exported,
            "{text:?}: kein Text hat sich geändert"
        );
        assert_eq!(revisions(&store).await, count, "{text:?}: keine Fassung angelegt");
    }
    // Text vor der Überschrift verweist auf den Editor.
    let edit = RevisionEdit { body: Some("Vorab.\n\n= Sommer\n\nA.\n"), note: None };
    store.update_revision(work, 3, 1, &edit, HUMAN).await.unwrap();
    let lead = store.switch_to(work, 3, 1, false, HUMAN).await.unwrap_err().to_string();
    assert!(lead.contains("im Editor"), "Hinweis auf den Editor: {lead}");
}

#[tokio::test]
async fn parts_switch_without_touching_the_outline() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let example = store.default_work().await.unwrap();
    let before = store.load_work(example).await.unwrap();
    let part = before.nodes.iter().position(|n| n.kind == baruch_core::NodeKind::Part).expect("Beispielwerk hat Teile");
    store.save_human(example, part, "Neuer Text des Teils.\n", HUMAN).await.unwrap().expect("neue Fassung");
    let first = before.nodes[part].revisions[0].number;
    let switch = store.switch_to(example, part, first, false, HUMAN).await.expect("Teile schalten ohne Prüfung um");
    assert!(switch.outline.is_none(), "Teile haben keine Überschriftszeile: die Gliederung bleibt");
    let after = store.load_work(example).await.unwrap();
    assert_eq!(levels(&after), levels(&before), "kein Teil wird umgestuft");
    assert_eq!(titles(&after), titles(&before), "kein Titel ändert sich");
}
