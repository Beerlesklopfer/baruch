// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 14 c „nummerierung“ (contract sections 3 and 4) in the store: chapters without number appear in export, export
//! map and mirror as the marking „ohne Nummer“, the texts in SQLite stay unchanged, `counted_headings` follows the
//! projection, and the numbers from a function of the work template are exact. Real SQLite file in the temp directory,
//! freely invented work.

use baruch_store::{Store, WorkId};

/// Invented work: template with numbers, three chapters, one section.
const WORK: &str = "#set text(lang: \"de\")\n#set heading(numbering: \"1.\")\n\
= Vorrede\nKurz.\n\
= Die Mühle\nDas Rad dreht sich.\n== Am Wehr\nWasser.\n\
= Der Müller\nMehl.\n";

const HUMAN: &str = "Autorin";

async fn imported() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(WORK, "muehle.typ", HUMAN).await.expect("Import").work;
    (dir, store, work)
}

/// All texts of all revisions of the work (key, number, text) – for „SQLite bleibt unverändert“.
async fn all_bodies(store: &Store, work: WorkId) -> Vec<(i64, i64, String)> {
    sqlx::query_as(
        "SELECT r.node_id, r.number, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? ORDER BY r.node_id, r.number",
    )
    .bind(work)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

async fn key_of(store: &Store, work: WorkId, title: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND title = ?")
        .bind(work)
        .bind(title)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn set_body(store: &Store, work: WorkId, title: &str, body: &str) {
    sqlx::query(
        "UPDATE revision SET body = ? WHERE status = 'current' AND node_id = \
         (SELECT id FROM node WHERE work_id = ? AND title = ?)",
    )
    .bind(body)
    .bind(work)
    .bind(title)
    .execute(store.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn a_chapter_without_number_is_a_marker_in_export_map_and_mirror_but_not_in_the_database() {
    let (_dir, store, work) = imported().await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        WORK,
        "frisch importiert: Export = Quelle"
    );
    store.outline_set_numbered(work, 0, false, HUMAN).await.expect("Vorrede ohne Nummer");
    let before = all_bodies(&store, work).await;
    let marker = format!("{}Kurz.\n", baruch_typst::unnumbered_line(1, "Vorrede"));
    for (scheme, custom) in [("template", None), ("decimal", None), ("legal", None), ("custom", Some("I.;a)"))] {
        if scheme != "template" {
            store.set_numbering_scheme(work, scheme, custom).await.unwrap();
        }
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
        assert!(exported.contains(&marker), "{scheme}: Vorrede als Markierung „ohne Nummer“: {exported}");
        assert!(!exported.contains("= Vorrede\n"), "{scheme}: keine =-Zeile mehr für die Vorrede");
        assert!(!exported.contains("#show heading.where"), "{scheme}: keine Show-Regel nötig");
        assert!(exported.contains("= Die Mühle\n"), "{scheme}: nummerierte Kapitel bleiben =-Zeilen");

        let map = store.export_map(work).await.unwrap();
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&map.typst),
            exported,
            "{scheme}: Export-Karte byte-gleich mit dem Export (eine Quelle)"
        );
        let first = &map.sections[0];
        assert!(
            baruch_typst::xref::strip_key_labels(&map.typst[first.start..first.end]).starts_with(&marker),
            "{scheme}: Bereich der Vorrede projiziert"
        );

        let files = store.mirror_snapshot(work).await.unwrap();
        let section = files.iter().find(|f| f.path.ends_with("-vorrede.typ")).expect("Datei der Vorrede");
        assert!(
            baruch_typst::xref::strip_key_labels(&section.contents).ends_with(&marker),
            "{scheme}: Spiegel projiziert: {}",
            section.contents
        );

        let counted = store.counted_headings(work).await.unwrap().expect("sicher bekannt");
        assert!(
            !counted.contains(&key_of(&store, work, "Vorrede").await),
            "{scheme}: Typst zählt die Markierung nicht, counted_headings auch nicht: {counted:?}"
        );
        assert_eq!(counted.len(), 2, "{scheme}: Mühle und Müller zählen");
        assert_eq!(all_bodies(&store, work).await, before, "{scheme}: alle Texte in SQLite unverändert");
    }
    store.set_numbering_scheme(work, "none", None).await.unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(exported.contains("= Vorrede\nKurz.\n"), "„Ohne Nummern“: keine Projektion, Text wie gespeichert");
    assert_eq!(store.counted_headings(work).await.unwrap(), Some(Vec::new()), "ohne Nummern zählt nichts");
}

#[tokio::test]
async fn a_marker_with_the_number_switched_on_is_a_heading_line_again() {
    // The other direction: if the text contains a marking „ohne Nummer“ but the node is counted (numbered = 1), the
    // projection sets the = line (node.numbered is authoritative, contract §2).
    let (_dir, store, work) = imported().await;
    let marked = format!("{}Mehl.\n", baruch_typst::unnumbered_line(1, "Der Müller"));
    set_body(&store, work, "Der Müller", &marked).await;
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(exported.ends_with("= Der Müller\nMehl.\n"), "Markierung wird =-Zeile: {exported}");
    let counted = store.counted_headings(work).await.unwrap().expect("sicher bekannt");
    assert!(counted.contains(&key_of(&store, work, "Der Müller").await), "und zählt: {counted:?}");
    assert_eq!(
        all_bodies(&store, work).await.iter().filter(|b| b.2 == marked).count(),
        1,
        "SQLite behält die Markierung"
    );

    store.outline_set_numbered(work, 3, false, HUMAN).await.expect("Müller ohne Nummer");
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(exported.ends_with(&marked), "passt die Textform, bleibt der Text byte-gleich: {exported}");
}

#[tokio::test]
async fn counted_headings_follow_the_fallback_and_same_titles() {
    let (_dir, store, work) = imported().await;
    // Same heading: Round 6 left out the rule and counted both; the marking hits only the Vorrede.
    set_body(&store, work, "Der Müller", "= Vorrede\nMehl.\n").await;
    store.outline_set_numbered(work, 0, false, HUMAN).await.unwrap();
    let counted = store.counted_headings(work).await.unwrap().unwrap();
    let preface = key_of(&store, work, "Vorrede").await;
    let miller = key_of(&store, work, "Der Müller").await;
    assert!(!counted.contains(&preface) && counted.contains(&miller), "nur die Vorrede zählt nicht: {counted:?}");

    // Own label: fallback show rule; the heading then is not counted either.
    set_body(&store, work, "Vorrede", "= Vorrede <vor>\nKurz.\n").await;
    set_body(&store, work, "Der Müller", "= Der Müller\nMehl.\n").await;
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(
        exported.contains("#show heading.where(level: 1, body: [Vorrede]): set heading(numbering: none)\n"),
        "Show-Regel für die Überschrift mit Label: {exported}"
    );
    let counted = store.counted_headings(work).await.unwrap().unwrap();
    assert!(!counted.contains(&preface), "mit Show-Regel zählt Typst sie nicht: {counted:?}");
    assert_eq!(counted.len(), 2, "Mühle und Müller");
}

#[tokio::test]
async fn numbers_from_a_template_function_are_exact_in_the_outline() {
    // Contract §3: a function of the work template is evaluated per level (up to Round 13 only the first numbering
    // pattern for all levels, here „1.1“ instead of „Kapitel 1“).
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let source = "#set heading(numbering: (..n) => if n.pos().len() == 1 { \"Kapitel \" + str(n.pos().at(0)) } \
                  else { numbering(\"1.a\", ..n) })\n= Eins\n== Unter\n= Zwei\n";
    let work = store.import_typst(source, "funktion.typ", HUMAN).await.unwrap().work;
    let numbers: Vec<String> = store.load_work(work).await.unwrap().nodes.into_iter().map(|n| n.number).collect();
    assert_eq!(numbers, ["Kapitel 1", "1.a", "Kapitel 2"], "exakt je Ebene wie Typst");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        source,
        "Export bleibt byte-gleich"
    );
}
