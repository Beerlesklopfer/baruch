// SPDX-License-Identifier: AGPL-3.0-or-later
//! Jump labels `<kN>` in the export, in the export map and in the mirror, not in the database; importing an export
//! removes them and maps references to the new keys (Round 30a, contract §3.3). Texts freely invented.

use baruch_store::Store;
use baruch_typst::xref::{anchor_line, find_xrefs, strip_key_labels, xref_snippet};
use baruch_typst::{part_line, unnumbered_line};

/// Work with part, „ohne Nummer“, own label and references to the part and the chapter (keys from an earlier export: 70
/// and 71).
fn source() -> String {
    format!(
        "#set heading(numbering: \"1.1\")\n{}Ein Wort vorab.\n\n{}\n= Leuchtturm <k71>\n\nDer Leuchtturm blinkt.\n\n\
         == Wachturm <wache>\n\nSiehe {} und {}.\n",
        unnumbered_line(1, "Vorwort"),
        part_line("Vom Hafen"),
        xref_snippet(71, "den Leuchtturm", true),
        xref_snippet(70, "den Teil", false),
    )
}

#[tokio::test]
async fn export_map_and_mirror_carry_the_labels_but_the_database_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let plain = "= Leuchtturm // Notiz\n\nDer Leuchtturm blinkt.\n\n== Wachturm\nText.";
    let work = store.import_typst(plain, "turm.typ", "Ida").await.unwrap().work;
    let loaded = store.load_work(work).await.unwrap();
    let (a, b) = (loaded.nodes[0].key, loaded.nodes[1].key);

    let exported = store.export_typst(work).await.unwrap();
    assert_eq!(
        exported,
        format!("= Leuchtturm <k{a}> // Notiz\n\nDer Leuchtturm blinkt.\n\n== Wachturm <k{b}>\nText."),
        "jede Überschrift trägt ihre feste Sprungmarke (vor dem Kommentar)"
    );
    assert_eq!(strip_key_labels(&exported), plain, "ohne Sprungmarken byte-gleich mit der Quelle");
    let map = store.export_map(work).await.unwrap();
    assert_eq!(map.typst, exported, "Karte und Export bleiben byte-gleich");
    let files = store.mirror_snapshot(work).await.unwrap();
    let section = files.iter().find(|f| f.path.starts_with("abschnitte/000")).unwrap();
    assert!(section.contents.starts_with(&format!("= Leuchtturm <k{a}>")), "Spiegel trägt sie: {}", section.contents);
    let stored = &loaded.nodes[0].revisions[0].body;
    assert!(!stored.contains("<k"), "die Datenbank bleibt ohne Sprungmarke: {stored}");
}

#[tokio::test]
async fn headings_with_their_own_label_get_an_anchor_line() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let text = format!("{}\n= Leuchtturm\n\nText.\n\n== Wachturm <wache>\n\nMehr.\n", part_line("Vom Hafen"));
    let work = store.import_typst(&text, "hafen.typ", "Ida").await.unwrap().work;
    let loaded = store.load_work(work).await.unwrap();
    let keys: Vec<i64> = loaded.nodes.iter().map(|n| n.key).collect();
    let exported = store.export_typst(work).await.unwrap();
    assert!(
        exported.contains(&format!("{}{}\n", part_line("Vom Hafen"), anchor_line(keys[0]))),
        "Teil: Ankerzeile direkt darunter: {exported}"
    );
    assert!(
        exported.contains(&format!("== Wachturm <wache>\n{}\n", anchor_line(keys[2]))),
        "eigenes Label bleibt, Ankerzeile darunter: {exported}"
    );
    assert_eq!(baruch_typst::parse(&exported).unwrap().headings.len(), 3, "die Gliederung bleibt dieselbe");
    assert_eq!(strip_key_labels(&exported), text, "ohne Sprungmarken byte-gleich");
}

#[tokio::test]
async fn importing_an_export_strips_labels_and_remaps_references() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    // The source comes from a foreign export: jump labels k70 (part, through the anchor line) and k71 (chapter).
    let text = source().replace(&part_line("Vom Hafen"), &format!("{}{}\n", part_line("Vom Hafen"), anchor_line(70)));
    let work = store.import_typst(&text, "hafen.typ", "Ida").await.unwrap().work;
    let loaded = store.load_work(work).await.unwrap();
    let titles: Vec<&str> = loaded.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["Vorwort", "Vom Hafen", "Leuchtturm", "Wachturm"], "Titel ohne Sprungmarke");
    let part = loaded.nodes[1].key;
    let chapter = loaded.nodes[2].key;
    let bodies: Vec<&str> = loaded.nodes.iter().map(|n| n.revisions[0].body.as_str()).collect();
    assert_eq!(bodies[1], format!("{}\n", part_line("Vom Hafen")), "Ankerzeile entfernt: {:?}", bodies[1]);
    assert!(bodies[2].starts_with("= Leuchtturm\n"), "Sprungmarke an der Überschrift entfernt: {:?}", bodies[2]);
    let uses = find_xrefs(bodies[3]);
    assert_eq!(
        uses.iter().map(|u| (u.key, u.page)).collect::<Vec<_>>(),
        vec![(chapter, true), (part, false)],
        "Verweise zeigen auf die neuen Schlüssel, auch im Seitenzahl-Zusatz: {}",
        bodies[3]
    );

    // Round trip: the export of this work imported again yields the same texts with new keys.
    let exported = store.export_typst(work).await.unwrap();
    let again =
        store.import_typst_titled(&exported, "hafen2.typ", "Ida", "Import", Some("Hafen 2")).await.unwrap().work;
    let second = store.load_work(again).await.unwrap();
    let new_chapter = second.nodes[2].key;
    assert_ne!(new_chapter, chapter, "neues Werk, neue Schlüssel");
    let body = &second.nodes[3].revisions[0].body;
    assert_eq!(find_xrefs(body)[0].key, new_chapter, "der Verweis folgt dem neuen Schlüssel: {body}");
    assert_eq!(
        second.nodes[2].revisions[0].body, loaded.nodes[2].revisions[0].body,
        "Text ohne Sprungmarke wie im ersten Werk"
    );
}
